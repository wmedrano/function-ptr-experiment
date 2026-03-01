use criterion::{Criterion, black_box, criterion_group, criterion_main};
use interpreter::{Val, Vm, make_adder, make_fib};

fn run_benches(c: &mut Criterion) {
    let mut vm = Vm::new();
    let fib = black_box(make_fib());
    c.bench_function("fib(12)", |b| {
        b.iter(|| vm.run_with_func(fib.clone(), black_box([Val::Int(12)])))
    });

    let adder = black_box(make_adder());
    c.bench_function("add(1, 2, 3, 4)", |b| {
        b.iter(|| {
            vm.run_with_func(
                adder.clone(),
                black_box([Val::Int(1), Val::Int(2), Val::Int(3), Val::Int(4)]),
            )
        })
    });
}

criterion_group!(benches, run_benches);
criterion_main!(benches);
