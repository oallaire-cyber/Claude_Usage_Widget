fn main() {
    cuw_bridge::main_entry();
    // Always exit 0: a failing status-line command must never disturb Claude Code.
    std::process::exit(0);
}
