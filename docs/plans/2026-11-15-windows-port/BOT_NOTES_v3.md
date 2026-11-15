# Bot notes for v3（Joe 01:44 回覆＋settings.json）
- Joe 試貼去 Chrome；講完自己撳 Ctrl+V 都冇呢段字；hotkey Ctrl+/（hotkey_mode=hold）。
- Windows settings.json：output_mode=keyboard、insertion_strategy=auto、paste_shortcut=ctrlV、restore_clipboard_after_paste=true、streaming_insert_enabled=false。
- log：`Output completed with status=Inserted, strategy=Keyboard, chars_inserted=3`，paste 13–14ms。
- 結論：Windows 預設行 output/keyboard.rs `type_text_sync`（enigo `text()`，spawn_blocking），完全唔經 clipboard，所以「restore 太快」唔係根因。
- 要查：enigo 喺 Windows 打 CJK unicode 去 Chrome 有冇效；hold Ctrl+/ 放手時 Ctrl 狀態（windows_modifier_guard L37/L59）；native_hotkey L1565 LL hook 會唔會食 enigo injected key（enigo 冇用 OwnKeystroke）；Windows Auto 應唔應該改行 WindowsSendInput（windows_sendinput.rs 有 OwnKeystroke＋chunk）或者 clipboard paste。
- log 16:28–16:29 有 `Shortcut gate: dictation`，嗰幾次可能係喺 OraType 自己嘅 shortcut 練習頁試，唔係 Chrome。
