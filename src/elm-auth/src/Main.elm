module Main exposing (main)

import Browser
import Browser.Navigation
import Element exposing (Element)
import Element.Background
import Element.Border
import Element.Font
import Element.Input
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


backgroundColor : Element.Color
backgroundColor =
    Element.rgb255 7 54 66


fontColor : Element.Color
fontColor =
    Element.rgb255 253 246 227


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


wrapper : Element msg -> Element msg
wrapper body =
    Element.el
        [ Element.Font.color fontColor
        , Element.Font.family
            [ Element.Font.typeface "SF Mono"
            , Element.Font.typeface "Menlo"
            , Element.Font.typeface "Andale Mono"
            , Element.Font.typeface "Monaco"
            , Element.Font.monospace
            ]
        , Element.Background.color backgroundColor
        , Element.height Element.fill
        , Element.width Element.fill
        , Element.padding 10
        ]
        body


onEnter : msg -> Element.Attribute msg
onEnter msg =
    Element.htmlAttribute
        (Html.Events.on "keyup"
            (Json.Decode.field "key" Json.Decode.string
                |> Json.Decode.andThen
                    (\key ->
                        if key == "Enter" then
                            Json.Decode.succeed msg

                        else
                            Json.Decode.fail "Not the enter key"
                    )
            )
        )


view : Model -> Element.Element Msg
view model =
    case model of
        Typing password ->
            Element.row
                [ Element.width Element.fill, Element.spacing 10 ]
                [ Element.Input.currentPassword
                    [ onEnter Submit
                    , Element.htmlAttribute (Html.Attributes.autofocus True)
                    , Element.Background.color backgroundColor
                    , Element.Border.color fontColor
                    , Element.width Element.fill

                    -- Fixes a bug in Safari
                    -- https://bugs.webkit.org/show_bug.cgi?id=142968
                    , Element.htmlAttribute (Html.Attributes.placeholder " ")
                    ]
                    { onChange = UpdatePassword
                    , placeholder = Nothing
                    , label = Element.Input.labelLeft [] (Element.text "Password:")
                    , show = False
                    , text = password
                    }
                , Element.Input.button
                    [ Element.Border.color fontColor
                    , Element.Border.width 1
                    , Element.Border.rounded 5
                    , Element.height Element.fill
                    , Element.paddingXY 5 0
                    ]
                    { onPress = Just Submit, label = Element.text "Enter" }
                ]

        Loading ->
            Element.text "Loading..."

        Success ->
            Element.text "Success!"

        Failure failure ->
            let
                error =
                    case failure of
                        ServerError ->
                            "Server error"

                        InvalidPassword ->
                            "Invalid password"
            in
            Element.text error


main : Program () Model Msg
main =
    Browser.document
        { init = always ( Typing "", Cmd.none )
        , view =
            \model ->
                { title = "Light the lantern"
                , body = [ Element.layout [] <| wrapper <| view model ]
                }
        , update = update
        , subscriptions = always Sub.none
        }
