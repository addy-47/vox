# Disable missing vendored backends on macOS
set(GGML_BLAS OFF CACHE BOOL "Disable BLAS" FORCE)
set(GGML_METAL OFF CACHE BOOL "Disable Metal" FORCE)
