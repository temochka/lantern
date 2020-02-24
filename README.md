# Lantern

*Lantern is alpha software. It contains bugs and shortcomings that will almost certainly make you mad. Don’t try to use it unless you’re a good stoic and have a lot of free time.*

Lantern is a lightweight backend for writing personal productivity apps. It handles authentication, serves static files, stores your data (in SQLite), and re-runs active SQL queries when the data changes. It exposes a WebSocket-based API that is designed for reactive UIs.

## Installation

Binaries for macOS, Linux (i386/amd64) are available from the releases tab on GitHub. The only dynamic dependency is libsqlite.

## Usage

Create and enter a directory for your first project:

``` bash
mkdir my-lantern-app
cd !$
```

Create your index file:

``` bash
echo "<html><head><title>My Lantern App</title></head><body>Lantern lit</body></html>" > index.html
```

Start Lantern in the current directory:

``` bash
$ LANTERN_PASSWORD=password lantern .
...lantern lit on port 4666, serving files from .
```

## API usage

TODO

## Bring your own schema

You can use Lantern with your existing SQLite-compatible schema. Create a new empty project, then create a `.lantern` directory:

``` bash
mkdir .lantern
```

Now copy your SQL schema file to this new directory:

``` bash
cp ~/my-old-backend/schema.sql .lantern/schema.sql
```

Start Lantern in the project root and it will load and normalize your schema.
