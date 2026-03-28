def check_stuff():
    print("Wait, what happens in loom model if `t1` runs first, fails to refresh, clears it. Then `t2` runs and sets it to 'new_rt'?")
    print("If `t1` clears it, then `t2` sets it to 'new_rt', the final state is 'new_rt'.")
    print("What if `t2` runs first, sets it to 'new_rt'? Then `t1` runs, sees 'new_rt', fails to refresh, and clears it? Then final state is `None`!")
    print("Ah! The test currently expects the final state to *always* be `Some('new_rt')`. But if `t2` runs before `t1`, `t1` reads 'new_rt', yields, and then clears it. And that's perfectly valid behavior because `t1` actually tried to refresh 'new_rt' and failed!")
check_stuff()
