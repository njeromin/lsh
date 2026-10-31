# VHS Tape for lsh demo

Output assets/demo.gif

Set Shell "bash"
Set FontFamily "JetBrainsMono Nerd Font"
Set FontSize 16
Set Width 900
Set Height 560
Set Padding 20
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set Margin 20
Set MarginFill "#1e1e2e"
Set TypingSpeed 45ms

# Launch lsh with local configuration
Hide
Type "cargo build --release -q"
Enter
Sleep 1s
Type "export LSH_CONFIG_DIR=./config; export LSH_NERD_ICONS=true; alias lsh=./target/release/lsh"
Enter
Type "clear"
Enter
Type "lsh"
Enter
Sleep 1s
Show

# 1. Custom prompt display and function definition
Type "fn greet(who) { return 'hello, ' + $who }"
Sleep 400ms
Enter
Sleep 1s

# 2. Structured command substitution (.out property)
Type "echo $(greet 'lsh').out"
Sleep 400ms
Enter
Sleep 1.5s

# 3. Object-based command status checking (.code property)
Type "if $(which cargo).code == 0 { echo 'cargo detected!' }"
Sleep 400ms
Enter
Sleep 1.5s

# 4. Interactive completion menu with live filtering & badges
Type "git "
Sleep 500ms
Tab
Sleep 1.5s
Type "co"
Sleep 1s
Down
Sleep 400ms
Up
Sleep 400ms
Enter
Sleep 600ms
Type " -"
Tab
Sleep 1.5s
Enter
Sleep 600ms
Type " 'feat: interactive completion'"
Sleep 1.5s
Ctrl+C
Sleep 800ms

# 5. Arrow redirection syntax
Type "echo 'structured non-posix shell' -> demo.txt"
Sleep 400ms
Enter
Sleep 1.2s

# 6. Unix pipeline chaining
Type "cat demo.txt | tr 'a-z' 'A-Z'"
Sleep 400ms
Enter
Sleep 1.5s

# 7. Scoped environment variable
Type "ENV_VAR=scoped echo $ENV_VAR"
Sleep 400ms
Enter
Sleep 2s

# Clean up temporary demo file
Hide
Type "rm demo.txt"
Enter
Type "exit"
Enter
Show

Sleep 2s
