# Chatty

[![codecov](https://codecov.io/gh/dmweis/chatty/branch/main/graph/badge.svg)](https://codecov.io/gh/dmweis/chatty)
[![Rust](https://github.com/dmweis/chatty/workflows/Rust/badge.svg)](https://github.com/dmweis/chatty/actions)
[![Private docs](https://github.com/dmweis/chatty/workflows/Deploy%20Docs%20to%20GitHub%20Pages/badge.svg)](https://davidweis.dev/chatty/chatty/index.html)

Small OpenAI API playground project

## API key

Get key from [OpenAI account](https://platform.openai.com/account/api-keys)

Save it to `configuration/dev_settings.yaml` as `open_ai_api_key`.

See example in `configuration/settings.yaml`

## ChatGPT cli

`cargo run --bin chatty` to run cli

non-exhaustive list of features:

* read user config
* save previous conversations
* title conversations using generated summary titles
