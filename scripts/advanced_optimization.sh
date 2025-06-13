#!/bin/bash

# Advanced macOS Optimization Script
# This script contains additional optimizations that complement the main Rust application

set -e

echo "🔧 Running advanced macOS optimizations..."

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to safely run commands with error handling
safe_run() {
    echo "Running: $*"
    if ! "$@"; then
        echo "⚠️  Warning: Command failed: $*"
        return 1
    fi
    return 0
}

# Memory and Performance Optimizations
echo "🧠 Optimizing memory and performance..."
safe_run sudo purge
safe_run sudo sysctl -w vm.pressure_disable_swap=1 2>/dev/null || true

# Network Optimizations
echo "🌐 Optimizing network settings..."
safe_run sudo dscacheutil -flushcache
safe_run sudo killall -HUP mDNSResponder

# Disk Optimization
echo "💾 Optimizing disk performance..."
safe_run sudo tmutil thinlocalsnapshots / 10000000000 4 2>/dev/null || true
safe_run sudo periodic daily weekly monthly

# Developer Tool Optimizations
echo "👨‍💻 Optimizing developer tools..."

# Xcode cleanup
if [ -d "$HOME/Library/Developer/Xcode" ] && command_exists xcode-select; then
    # Check if Xcode is properly installed
    if xcode-select -p >/dev/null 2>&1; then
        echo "Cleaning Xcode cache..."
        safe_run rm -rf "$HOME/Library/Developer/Xcode/DerivedData"
        safe_run rm -rf "$HOME/Library/Developer/Xcode/Archives"

        # Only run simctl if it's available
        if command_exists xcrun && xcrun simctl help >/dev/null 2>&1; then
            safe_run xcrun simctl delete unavailable
        else
            echo "⚠️  simctl not available, skipping simulator cleanup"
        fi
    else
        echo "⚠️  Xcode command line tools not properly configured, skipping Xcode cleanup"
    fi
fi

# Node.js cleanup
if command_exists npm; then
    echo "Cleaning npm cache..."
    safe_run npm cache clean --force
fi

# Docker cleanup
if command_exists docker; then
    echo "Cleaning Docker..."
    safe_run docker system prune -af --volumes 2>/dev/null || true
fi

# Homebrew optimizations
if command_exists brew; then
    echo "Optimizing Homebrew..."
    safe_run brew doctor || true
    safe_run brew cleanup --prune=all
    safe_run brew autoremove
fi

# System Cache Cleanup
echo "🗑️  Cleaning system caches..."
safe_run rm -rf "$HOME/Library/Caches/com.apple.Safari/WebKitCache" 2>/dev/null || true
safe_run rm -rf "$HOME/Library/Caches/Google/Chrome/Default/Cache" 2>/dev/null || true
safe_run rm -rf "$HOME/Library/Caches/Firefox/Profiles/"*/cache2 2>/dev/null || true

# Log Cleanup
echo "📝 Cleaning logs..."
safe_run sudo rm -rf /private/var/log/asl/*.asl 2>/dev/null || true
safe_run sudo rm -rf /Library/Logs/DiagnosticReports/* 2>/dev/null || true
safe_run rm -rf "$HOME/Library/Application Support/CrashReporter/"* 2>/dev/null || true

# Font Cache Rebuild
echo "🔤 Rebuilding font cache..."
safe_run sudo atsutil databases -remove
safe_run atsutil server -shutdown
safe_run atsutil server -ping

# Launch Services Rebuild
echo "🚀 Rebuilding Launch Services..."
safe_run /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -kill -r -domain local -domain system -domain user

# Spotlight Reindex (optional)
read -p "Do you want to reindex Spotlight? (y/N): " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "🔍 Reindexing Spotlight..."
    safe_run sudo mdutil -i off /
    safe_run sudo mdutil -E /
    safe_run sudo mdutil -i on /
fi

# System Integrity Check
echo "🔒 Running system integrity check..."
safe_run sudo /usr/libexec/locate.updatedb 2>/dev/null || true

# Final cleanup
echo "🧹 Final cleanup..."
safe_run sudo kextcache -system-prelinked-kernel 2>/dev/null || true
safe_run sudo kextcache -system-caches 2>/dev/null || true

echo "✅ Advanced optimizations complete!"
echo "💡 Consider restarting your Mac to apply all changes."
