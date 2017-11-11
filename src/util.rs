macro_rules! trace {
    ($($arg:tt)+)  => ({
        #[cfg(feature="debugtrace")]
        eprintln!($($arg)+);
    })
}
