module Main exposing (main)

import Browser
import Browser.Navigation
import Html
import Html.Attributes
import Html.Events
import Http
import Json.Decode
import Json.Encode
import Process
import Task


type AuthFailure
    = InvalidPassword
    | ServerError


type Model
    = Typing String
    | Loading
    | Success
    | Failure AuthFailure


type Msg
    = UpdatePassword String
    | HandleResponse (Result Http.Error ())
    | Submit
    | Reset


authenticate : String -> Cmd Msg
authenticate password =
    Http.post
        { url = "/_api/auth"
        , body =
            Json.Encode.object
                [ ( "password", Json.Encode.string password ) ]
                |> Json.Encode.encode 0
                |> Http.stringBody "application/json"
        , expect = Http.expectJson HandleResponse (Json.Decode.succeed ())
        }


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        Submit ->
            case model of
                Typing password ->
                    ( Loading, authenticate password )

                _ ->
                    ( model, Cmd.none )

        UpdatePassword password ->
            ( Typing password, Cmd.none )

        HandleResponse result ->
            case result of
                Err e ->
                    let
                        failure =
                            case e of
                                Http.BadStatus 422 ->
                                    Failure InvalidPassword

                                _ ->
                                    Failure ServerError
                    in
                    ( failure, Process.sleep 1000.0 |> Task.perform (always Reset) )

                Ok _ ->
                    ( Success, Browser.Navigation.reload )

        Reset ->
            ( Typing "", Cmd.none )


view : Model -> Html.Html Msg
view model =
    case model of
        Typing password ->
            Html.form [ Html.Events.onSubmit Submit ]
                [ Html.label [] [ Html.text "Password" ]
                , Html.input
                    [ Html.Attributes.type_ "password"
                    , Html.Attributes.value password
                    , Html.Events.onInput UpdatePassword
                    ]
                    []
                , Html.input [ Html.Attributes.type_ "submit" ] []
                ]

        Loading ->
            Html.div [] [ Html.text "Loading..." ]

        Success ->
            Html.div [] [ Html.text "Success!" ]

        Failure failure ->
            let
                error =
                    case failure of
                        ServerError ->
                            "Server error"

                        InvalidPassword ->
                            "Invalid password"
            in
            Html.div [] [ Html.p [] [ Html.text error ] ]


main : Program () Model Msg
main =
    Browser.document
        { init = always ( Typing "", Cmd.none )
        , view =
            \model ->
                { title = "Light the lantern"
                , body = [ view model ]
                }
        , update = update
        , subscriptions = always Sub.none
        }
