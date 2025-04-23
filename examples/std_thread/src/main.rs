// NEWLINE-TIMEOUT: 5
// ASSERT-SUCC: Done rust-std demo

#![feature(thread_id_value)]

use std::thread;

const NTHREADS: u32 = 4;

// This is the `main` thread
fn main() {
    println!("This is main thread {:x}", thread::current().id().as_u64());
    // Make a vector to hold the children which are spawned.
    let mut children = vec![];
    for _ in 0..NTHREADS {
        // Embedded devices don't have enough RAM to hold large stacks.
        let builder = thread::Builder::new().stack_size(1024);
        children.push(
            builder
                .spawn(move || {
                    println!("This is thread {:x}", thread::current().id().as_u64());
                })
                .unwrap(),
        );
    }

    for child in children {
        // Wait for the thread to finish. Returns a result.
        let tid = &child.thread().id().as_u64();
        println!("Joining thread {:x}", tid);
        let _ = child.join();
        println!("Joined thread {:x}", tid);
    }

    let blueos_logo = r#"
=====            ...
===== .*##*=:    #@@+                                    -*#%%%%#+:       .=*%@@%#*-
::::=:+++#@@@*   #@@+                                 .*@@@*+==+%@@@=    +@@@*++*#@@:
  :#@     +@@@   #@@+  ...      ...      .:--:       :@@@+       -@@@*  .@@@-      .
 :@@@    :#@@+   #@@+  %@@-    .@@@   .+@@@%@@@*.    %@@*         :@@@-  %@@@+-.
 -@@@@@@@@@%=    #@@+  %@@-    .@@@  .@@@-   .@@%   .@@@-          @@@*   +%@@@@@%+:
 -@@@::::-*@@@:  #@@+  %@@-    .@@@  +@@@*****%@@:   @@@=          @@@+     .-=*%@@@#
 -@@@      #@@#  #@@+  %@@-    :@@@  *@@#--------    *@@@.        *@@@.          -@@@:
 -@@@    .=@@@=  #@@+  *@@#.  :%@@@  .@@@=.    .      *@@@+-. .:=%@@@:  -@#+:.  .+@@@.
 -@@@@@@@@@@#-   #@@+   *@@@@@@+@@@   .*@@@@@@@@:      :*@@@@@@@@@#=    -*@@@@@@@@@#.
"#;
    println!("{}", blueos_logo);
    println!("Done rust-std demo");
}
