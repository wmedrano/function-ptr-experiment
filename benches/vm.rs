use criterion::{Criterion, black_box, criterion_group, criterion_main};
use interpreter::{Val, Vm, make_fib};

fn bench_fib(c: &mut Criterion) {
    let mut vm = Vm::new();
    let fib = black_box(make_fib());
    c.bench_function("fib(12)", |b| {
        b.iter(|| vm.run_with_func(fib.clone(), black_box([Val::Int(12)])))
    });
}

criterion_group!(benches, bench_fib);
criterion_main!(benches);
