# Third-party notices

Munia includes the following third-party software.

## Chiron GoRound TC

The Latin webfont subsets are by the Chiron GoRound TC Project Authors and distributed under
the [SIL Open Font License 1.1](src/brand/fonts/OFL.txt).

## whisper.cpp, llama.cpp and ggml

whisper.cpp is used for built-in speech recognition (through the `whisper-rs` crate, which is
released under the Unlicense). llama.cpp's `llama-server` (release b11177) is built from source
and shipped inside Munia for Built-in AI polish. Both are released by the ggml authors under
the MIT License (llama.cpp: Copyright (c) 2023-2026 The ggml authors).

MIT License

Copyright (c) 2023-2024 The ggml authors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## Libraries inside llama-server

`llama-server` includes cpp-httplib (Copyright (c) 2017 yhirose) and JSON for Modern C++
(Copyright (c) 2013-2025 Niels Lohmann), both under the MIT License above.

## Speech models

Whisper model files downloaded by Quick setup and by Hands-free mode (the base model used for
the wake phrase) come from the whisper.cpp Hugging Face repository
and are released by OpenAI under the MIT License.

The Qwen3-ASR model and audio projector files downloaded by Built-in speech are
Qwen3-ASR-1.7B-HK-Cantonese, Koelony's fine-tune of the Qwen team's Qwen3-ASR-1.7B (a
Q6_K model and Q8_0 projector GGUF from the koelony/Qwen3-ASR-1.7B-Cantonese-HK-GGUF Hugging Face
repository), and Qwen3-ASR-0.6B-HK-Cantonese, Koelony's fine-tune of the Qwen team's
Qwen3-ASR-0.6B (a Q6_K model and Q8_0 projector GGUF from the
koelony/Qwen3-ASR-0.6B-Cantonese-HK-GGUF Hugging Face repository). Earlier releases downloaded
the stock Qwen3-ASR-1.7B. Version 1.2.1 downloaded a Q4_K_M model and Q8_0 projector from
mradermacher's repository, and versions 1.1.3 through 1.2.0 downloaded Q8_0 files from ggml-org's. All of them are released under the Apache License 2.0
(https://www.apache.org/licenses/LICENSE-2.0). They are downloaded, not shipped with Munia.

## AI models

The Qwen3 and Qwen 3.5 model files downloaded by Built-in AI (Qwen3-4B-Instruct-2507 and Qwen3-1.7B
as Q4_K_M, and Qwen3.5-4B as Q4_0, GGUF files from Unsloth's Hugging Face repositories) are released by the Qwen team under
the Apache License 2.0 (https://www.apache.org/licenses/LICENSE-2.0). They are downloaded, not
shipped with Munia.

## Evaluation data

The Cantonese speech evaluation (`evals/speech/yue/`) uses clips from two public sets. The
FLEURS clips (Conneau et al., 2022, https://huggingface.co/datasets/google/fleurs) are released
by Google under the Creative Commons Attribution 4.0 licence
(https://creativecommons.org/licenses/by/4.0/); their transcripts are copied into the manifest
and the audio is downloaded by `npm run eval -- fetch`, not shipped with Munia. The Common Voice
zh-HK clips (Mozilla, https://commonvoice.mozilla.org/) are released under CC0 1.0.
