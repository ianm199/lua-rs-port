#!/usr/bin/env bash
# with-timeout.sh <seconds> <command...> — run a command with a hard watchdog
# that kills the WHOLE process group on expiry (exit 142).
#
# Why not `perl -e 'alarm shift; exec @ARGV'`: alarm survives exec but only
# kills the exec'd process itself. With `/usr/bin/time <bin> ...` the alarm
# kills time(1) and ORPHANS the grandchild interpreter, which keeps spinning
# at 100% CPU forever — two such orphans contaminated benchmarks for hours on
# 2026-06-09. This wrapper forks the command into its own process group and
# kills the group, so children of children die too. macOS has no timeout(1).
set -u
exec perl -e '
    my $t = shift @ARGV;
    my $pid = fork() // die "fork: $!";
    if ($pid == 0) {
        setpgrp(0, 0);
        exec @ARGV or exit 127;
    }
    $SIG{ALRM} = sub { kill 9, -$pid; waitpid($pid, 0); exit 142; };
    alarm $t;
    waitpid($pid, 0);
    my $st = $?;
    exit(($st & 127) ? 128 + ($st & 127) : $st >> 8);
' "$@"
