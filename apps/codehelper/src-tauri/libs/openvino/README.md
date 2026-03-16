This directory is tracked so clean checkouts and CI have a real
`libs/openvino/` path for Tauri resource resolution.

The actual Windows OpenVINO runtime bundle is not checked into git. Populate
this directory on Windows with:

- `npm run runtime:setup:openvino`

That script stages the required OpenVINO GenAI DLLs into this directory for
development and packaging.
