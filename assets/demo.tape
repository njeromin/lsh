# VHS Tape for lsh demo
# Run with: nix run nixpkgs#vhs assets/demo.tape (or `vhs assets/demo.tape`)

Output assets/demo.gif

Set Shell "bash"
Set FontSize 16
Set Width 900
Set Height 520
Set Padding 20
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set Margin 20
Set MarginFill "#1e1e2e"
Set TypingSpeed 45ms

# Launch lsh with local configuration
Hide
Type "cargo build -q"
Enter
Sleep 1s
Type "export LSH_CONFIG_DIR=./config"
Enter
Type "clear"
Enter
Type "./target/debug/lsh"
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

# 4. Arrow redirection syntax
Type "echo 'structured non-posix shell' -> demo.txt"
Sleep 400ms
Enter
Sleep 1.2s

# 5. Unix pipeline chaining
Type "cat demo.txt | tr 'a-z' 'A-Z'"
Sleep 400ms
Enter
Sleep 1.5s

# 6. Scoped environment variable
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
