# study-rust-async

This is just some Rust I was playing around with over xmas 2019.

I was interested in using Rust `async` in my bigger projects but it
wasn't clear to me how `Future<T>`'s actually work, how they're
executed, and so on. I decided to try and write a futures "framework"
from scratch, using only the interfaces available in the standard
library (no `tokio`, `futures`, etc.).

Turns out that writing an efficient `Future<T>` can get *quite*
complicated compared to the usual GC-language's deal of just calling
some callback.
