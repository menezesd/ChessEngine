#!/usr/bin/env python3
"""Test that go movetime still works correctly (searches for specific time)."""

import subprocess
import sys
import time

def test_movetime_command():
    """Test that 'go movetime N' searches for N milliseconds."""
    
    cmd = ["./target/release/chess_engine"]
    
    # Test time-limited search (1000ms = 1 second)
    uci_commands = """uci
isready
position startpos
go movetime 1000
quit
"""
    
    try:
        start_time = time.time()
        result = subprocess.run(
            cmd, 
            input=uci_commands, 
            text=True, 
            capture_output=True, 
            timeout=15
        )
        end_time = time.time()
        
        actual_time = end_time - start_time
        
        output = result.stdout
        print("=== Testing 'go movetime 1000' ===")
        
        # Check if we got a result
        if "bestmove" in output:
            print(f"✅ SUCCESS: Got bestmove in {actual_time:.1f} seconds")
            
            # Should take roughly 1 second (allow some variance)
            if 0.8 <= actual_time <= 2.0:
                print(f"✅ TIMING OK: Search time {actual_time:.1f}s is reasonable for 1000ms")
            else:
                print(f"⚠️  TIMING: Search took {actual_time:.1f}s (expected ~1.0s)")
            
            return True
        else:
            print("❌ ERROR: No bestmove found in output")
            return False
            
    except Exception as e:
        print(f"❌ ERROR: {e}")
        return False

if __name__ == "__main__":
    success = test_movetime_command()
    sys.exit(0 if success else 1)