use interpreter::{Val, Vm, make_fib};

fn main() {
    let mut vm = Vm::new();
    let fib = make_fib();
    let n = 40;
    let ans = vm.run_with_func(fib, [Val::Int(n)]).unwrap();
    println!("fib({n}) = {ans:?}");
}
