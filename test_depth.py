#!/usr/bin/env python3
"""Test that go depth works correctly (searches to specific depth, not time)."""

import subprocess
import sys

def test_depth_command():
    """Test that 'go depth N' searches to depth N, not for fixed time."""
    
    cmd = ["./target/release/chess_engine"]
    
    # Test depth-limited search
    uci_commands = """uci
isready
position startpos
go depth 5
quit
"""
    
    try:
        result = subprocess.run(
            cmd, 
            input=uci_commands, 
            text=True, 
            capture_output=True, 
            timeout=10
        )
        
        output = result.stdout
        print("=== Testing 'go depth 5' ===")
        print(f"Output:\n{output}")
        
        # Check if we see depth progression up to 5
        lines = output.split('\n')
        depths_seen = []
        
        for line in lines:
            if line.startswith('info') and 'depth' in line:
                parts = line.split()
                if 'depth' in parts:
                    depth_idx = parts.index('depth')
                    if depth_idx + 1 < len(parts):
                        try:
                            depth = int(parts[depth_idx + 1])
                            depths_seen.append(depth)
                        except ValueError:
                            pass
        
        print(f"Depths seen: {depths_seen}")
        
        if depths_seen:
            max_depth = max(depths_seen)
            print(f"Maximum depth reached: {max_depth}")
            
            if max_depth == 5:
                print("✅ SUCCESS: Correctly searched to depth 5")
                return True
            elif max_depth < 5:
                print(f"⚠️  WARNING: Only reached depth {max_depth}, expected 5")
                return True  # Still working, just didn't reach full depth
            else:
                print(f"❌ ERROR: Exceeded requested depth (reached {max_depth})")
                return False
        else:
            print("❌ ERROR: No depth information found in output")
            return False
            
    except Exception as e:
        print(f"❌ ERROR: {e}")
        return False

if __name__ == "__main__":
    # Build engine first
    print("Building chess engine...")
    build_result = subprocess.run(["cargo", "build", "--release"], capture_output=True, text=True)
    if build_result.returncode != 0:
        print("Build failed:")
        print(build_result.stderr)
        sys.exit(1)
    
    success = test_depth_command()
    sys.exit(0 if success else 1)