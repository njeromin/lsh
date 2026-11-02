use std::collections::HashMap;

use crate::types::{CandidateSpec, CompletionKind, CompletionRule};

pub fn populate_ssh_rules(rules: &mut HashMap<String, CompletionRule>) {
    let mut ssh_rule = CompletionRule::new();
    ssh_rule.candidates = vec![
        CandidateSpec::from_raw(
            "-p:Port to connect to on the remote host",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-i:Select identity (private key) file",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-v:Verbose mode (debug connection)", CompletionKind::Flag),
        CandidateSpec::from_raw("-vv:More verbose debugging", CompletionKind::Flag),
        CandidateSpec::from_raw("-vvv:Full verbose debugging", CompletionKind::Flag),
        CandidateSpec::from_raw("-X:Enable X11 forwarding", CompletionKind::Flag),
        CandidateSpec::from_raw("-x:Disable X11 forwarding", CompletionKind::Flag),
        CandidateSpec::from_raw("-Y:Enable trusted X11 forwarding", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-A:Enable authentication agent forwarding",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-a:Disable authentication agent forwarding",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-N:Do not execute a remote command (port forwarding only)",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-f:Requests ssh to go to background just before execution",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-n:Redirects stdin from /dev/null", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-T:Disable pseudo-terminal allocation",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-t:Force pseudo-terminal allocation", CompletionKind::Flag),
        CandidateSpec::from_raw("-L:Local port forwarding", CompletionKind::Flag),
        CandidateSpec::from_raw("-R:Remote port forwarding", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-D:Dynamic application-level port forwarding",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-C:Enable compression", CompletionKind::Flag),
        CandidateSpec::from_raw("-q:Quiet mode (suppress warnings)", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-4:Forces ssh to use IPv4 addresses only",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw(
            "-6:Forces ssh to use IPv6 addresses only",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-l:Login name on remote machine", CompletionKind::Flag),
        CandidateSpec::from_raw("-o:Option in ssh_config format", CompletionKind::Flag),
        CandidateSpec::from_raw(
            "-F:Alternative user configuration file",
            CompletionKind::Flag,
        ),
        CandidateSpec::from_raw("-V:Display version number and exit", CompletionKind::Flag),
    ];
    ssh_rule.exec = Some("{ awk '/^[[:space:]]*Host[[:space:]]+/ {for (i=2; i<=NF; i++) if ($i !~ /[*?]/) print $i\":SSH Config Host\"}' ~/.ssh/config 2>/dev/null; awk '{split($1, a, \",\"); for (i in a) if (a[i] !~ /^\\|/ && a[i] !~ /[*?]/) print a[i]\":Known Host\"}' ~/.ssh/known_hosts 2>/dev/null; } | awk -F: '!seen[$1]++'".to_string());
    rules.insert("ssh".to_string(), ssh_rule);
}

pub use populate_ssh_rules as populate_rules;
