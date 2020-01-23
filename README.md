# Lantern

Lantern is a backend for your personal productivity apps. It handles authentication, serves your static files, stores your data, and re-runs your SQL queries when the data changes. It exposes a WebSocket-based API that is built with reactive UIs in mind.

## Bring your own schema

Start Lantern in the current directory:

```
$ lantern --master-password=password

...lantern lit on port 4666, serving files from .
```
