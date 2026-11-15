if (WIN32 AND MINGW)
    cmake_language(DEFER CALL set_target_properties ggml ggml-base ggml-cpu PROPERTIES PREFIX "lib")
endif()
