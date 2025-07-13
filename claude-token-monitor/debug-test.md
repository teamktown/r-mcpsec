# Debugging Tab Key Issue

## How to Test with Debug Output

1. **Run the application:**
   ```bash
   cargo run --release
   ```

2. **Watch for debug output** - All debug messages will appear on stderr with `🔍 DEBUG:` prefix

3. **Test tab key specifically:**
   - Press Tab key while on Overview tab (tab 0)
   - Look for these debug messages:
     ```
     🔍 DEBUG: Key event - code: Tab, modifiers: ..., current_tab: 0
     🔍 DEBUG: Tab key pressed - changed from tab 0 to tab 1
     ```

4. **Expected vs Actual Behavior:**
   - **Expected:** Tab should switch from Overview (0) → Charts (1) → Session (2) → etc.
   - **If bug exists:** App exits instead of switching tabs

## Debug Messages to Watch For

### Normal Tab Switching:
```
🔍 DEBUG: Main UI loop iteration - current_tab: 0, should_exit: false
🔍 DEBUG: Key event - code: Tab, modifiers: ..., current_tab: 0
🔍 DEBUG: Tab key pressed - changed from tab 0 to tab 1
🔍 DEBUG: handle_input returned: false
🔍 DEBUG: Main UI loop iteration - current_tab: 1, should_exit: false
```

### If App Exits Unexpectedly:
```
🔍 DEBUG: Main UI loop iteration - current_tab: 0, should_exit: false
🔍 DEBUG: Key event - code: ?, modifiers: ?, current_tab: 0
🔍 DEBUG: Quit key pressed, exiting application  <-- Unexpected!
🔍 DEBUG: handle_input returned: true
🔍 DEBUG: Breaking from main loop due to handle_input returning true
```

## Possible Issues to Look For

1. **Wrong Key Code:** Tab might be interpreted as something else
2. **Modifier Keys:** Shift+Tab or other modifiers might trigger exit
3. **Terminal Issues:** Some terminals might send different key codes
4. **Crossterm Event Handling:** Event parsing might be incorrect

## Other Keys to Test

- **'v'** - Should toggle Overview view mode (only on tab 0)
- **Arrow keys** - Should not exit
- **'q' or Esc** - Should exit (this is expected)
- **Ctrl+C** - Should exit (this is expected)

## Quick Fix Test

If Tab is being misinterpreted, try these alternative keys:
- **Right arrow** - Should work normally
- **'n'** - We could add this as an alternative tab switch

The debugging output will show exactly what key codes are being received and how they're being processed.