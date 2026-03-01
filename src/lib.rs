mod func;
mod val;
pub use func::{Binop, Func, Instruction};
pub use val::{Symbol, Val};

pub(crate) type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during VM execution.
#[derive(Debug, PartialEq)]
pub enum Error {
    /// A function was called with the wrong number of arguments.
    WrongArgCount { expected: usize, got: usize },
    /// A value had an unexpected type.
    WrongType {
        expected: &'static str,
        got: &'static str,
    },
    /// The stack was empty when a value was expected.
    StackUnderflow,
    /// Division by zero.
    DivideByZero,
    /// A symbol was too long.
    SymbolTooLong { len: usize },
}

/// A stack-based bytecode virtual machine.
#[derive(Debug)]
pub struct Vm {
    pub(crate) stack: Vec<Val>,
    stack_frames: Vec<StackFrame>,
}

#[derive(Debug)]
pub struct StackFrame {
    stack_start: usize,
    instruction_idx: usize,
    func: Func,
}

impl Default for Vm {
    fn default() -> Self {
        Self::new()
    }
}

impl Vm {
    /// Create a new VM with empty state.
    pub fn new() -> Vm {
        Vm {
            stack: Vec::with_capacity(128),
            stack_frames: Vec::with_capacity(64),
        }
    }

    /// Call a function value with the given arguments and return the result.
    pub fn run_with_func(
        &mut self,
        func_val: Val,
        args: impl IntoIterator<Item = Val>,
    ) -> Result<Val> {
        self.stack.clear();
        self.stack_frames.clear();
        self.stack.extend(args);

        match func_val {
            Val::Func(func) => {
                let arg_count = self.stack.len();
                if func.args() != arg_count {
                    return Err(Error::WrongArgCount {
                        expected: func.args(),
                        got: arg_count,
                    });
                }
                self.stack_frames.push(StackFrame {
                    stack_start: 0,
                    instruction_idx: 0,
                    func,
                });
                self.run()
            }
            Val::NativeFunc(nf) => (nf.0)(&self.stack),
            _ => Err(Error::WrongType {
                expected: "Func or NativeFunc",
                got: func_val.type_name(),
            }),
        }
    }

    fn run(&mut self) -> Result<Val> {
        let mut current_frame = self.stack_frames.pop().ok_or(Error::StackUnderflow)?;
        loop {
            let instruction = current_frame.func.instructions()[current_frame.instruction_idx];
            current_frame.instruction_idx += 1;
            match instruction {
                Instruction::Eval(n) => {
                    if n >= 0 {
                        let arg_count = n as usize;
                        let func_val = self.stack.pop().ok_or(Error::StackUnderflow)?;
                        match func_val {
                            Val::Func(f) => {
                                self.execute_eval(&mut current_frame, f, arg_count)?;
                            }
                            Val::NativeFunc(nf) => {
                                if self.stack.len() < arg_count {
                                    return Err(Error::StackUnderflow);
                                }
                                let stack_start = self.stack.len() - arg_count;
                                let result = (nf.0)(&self.stack[stack_start..])?;
                                self.stack.truncate(stack_start);
                                self.stack.push(result);
                            }
                            v => {
                                return Err(Error::WrongType {
                                    expected: "Func",
                                    got: v.type_name(),
                                });
                            }
                        }
                    } else {
                        let arg_count = (n as u8 & 0x7F) as usize;
                        let func = current_frame.func.clone();
                        self.execute_eval(&mut current_frame, func, arg_count)?;
                    }
                }
                Instruction::LoadInt(x) => {
                    self.stack.push((x as i64).into());
                }
                Instruction::LoadConst(idx) => {
                    let val = current_frame.func.constants()[idx as usize].clone();
                    self.stack.push(val);
                }
                Instruction::LoadLocal(idx) => {
                    let val = self.stack[current_frame.stack_start + idx as usize].clone();
                    self.stack.push(val);
                }
                Instruction::SetLocal(idx) => {
                    let val = self.stack.pop().ok_or(Error::StackUnderflow)?;
                    self.stack[current_frame.stack_start + idx as usize] = val;
                }
                Instruction::JumpIf(n) => {
                    let is_truthy = self.stack.pop().ok_or(Error::StackUnderflow)?.is_truthy();
                    if is_truthy {
                        current_frame.instruction_idx =
                            (current_frame.instruction_idx as isize + n as isize) as usize;
                    }
                }
                Instruction::Jump(n) => {
                    current_frame.instruction_idx =
                        (current_frame.instruction_idx as isize + n as isize) as usize;
                }
                Instruction::Return => {
                    let stack_start = current_frame.stack_start;
                    let last = self
                        .stack
                        .len()
                        .checked_sub(1)
                        .ok_or(Error::StackUnderflow)?;
                    match self.stack_frames.pop() {
                        None => {
                            let val = self.stack.pop().ok_or(Error::StackUnderflow)?;
                            return Ok(val);
                        }
                        Some(frame) => {
                            self.stack.swap(stack_start, last);
                            self.stack.truncate(stack_start + 1);
                            current_frame = frame;
                        }
                    }
                }
                Instruction::Binop(op) => {
                    let a = self.stack.pop().ok_or(Error::StackUnderflow)?;
                    let top = self.stack.last_mut().ok_or(Error::StackUnderflow)?;
                    apply_binop(op, a, top)?;
                }
                Instruction::AddN(n) => {
                    let top = self.stack.last_mut().ok_or(Error::StackUnderflow)?;
                    match top {
                        Val::Int(x) => *x += n as i64,
                        _ => {
                            return Err(Error::WrongType {
                                expected: "Int",
                                got: top.type_name(),
                            });
                        }
                    }
                }
                Instruction::LessThan(n) => {
                    let top = self.stack.last_mut().ok_or(Error::StackUnderflow)?;
                    match top {
                        Val::Int(x) => {
                            let is_less_than = *x < n as i64;
                            *top = is_less_than.into();
                        }
                        _ => {
                            return Err(Error::WrongType {
                                expected: "Int",
                                got: top.type_name(),
                            });
                        }
                    }
                }
                Instruction::GreaterThan(n) => {
                    let top = self.stack.last_mut().ok_or(Error::StackUnderflow)?;
                    match top {
                        Val::Int(x) => {
                            let is_greater_than = *x > n as i64;
                            *top = is_greater_than.into();
                        }
                        _ => {
                            return Err(Error::WrongType {
                                expected: "Int",
                                got: top.type_name(),
                            });
                        }
                    }
                }
                Instruction::Equal(n) => {
                    let top = self.stack.last_mut().ok_or(Error::StackUnderflow)?;
                    match top {
                        Val::Int(x) => {
                            let is_equal = *x == n as i64;
                            *top = is_equal.into();
                        }
                        _ => {
                            return Err(Error::WrongType {
                                expected: "Int",
                                got: top.type_name(),
                            });
                        }
                    }
                }
                Instruction::StringLength => {
                    let top = self.stack.last_mut().ok_or(Error::StackUnderflow)?;
                    match top {
                        Val::String(s) => *top = Val::Int(s.len() as i64),
                        Val::Symbol(s) => *top = Val::Int(s.as_str().len() as i64),
                        _ => {
                            return Err(Error::WrongType {
                                expected: "Str or Symbol",
                                got: top.type_name(),
                            });
                        }
                    }
                }
                Instruction::Dup(n) => {
                    let val = self.stack.last().ok_or(Error::StackUnderflow)?.clone();
                    self.stack
                        .extend(std::iter::repeat_with(|| val.clone()).take(n as usize));
                }
            }
        }
    }

    #[inline(always)]
    fn execute_eval(
        &mut self,
        current_frame: &mut StackFrame,
        func: Func,
        arg_count: usize,
    ) -> Result<()> {
        let stack_start = self.stack.len() - arg_count;
        if func.args() != arg_count {
            return Err(Error::WrongArgCount {
                expected: func.args(),
                got: arg_count,
            });
        }
        let caller_frame = std::mem::replace(
            current_frame,
            StackFrame {
                stack_start,
                instruction_idx: 0,
                func,
            },
        );
        self.stack_frames.push(caller_frame);
        Ok(())
    }
}

fn apply_binop(op: Binop, a: Val, top: &mut Val) -> Result<()> {
    match op {
        Binop::Eq => {
            let res = &a == top;
            *top = Val::Bool(res);
        }
        Binop::NotEq => {
            let res = &a != top;
            *top = Val::Bool(res);
        }
        _ => {
            let Val::Int(av) = a else {
                return Err(Error::WrongType {
                    expected: "Int",
                    got: a.type_name(),
                });
            };
            let Val::Int(tv) = top else {
                return Err(Error::WrongType {
                    expected: "Int",
                    got: top.type_name(),
                });
            };
            match op {
                Binop::Add => *tv += av,
                Binop::Sub => *tv -= av,
                Binop::Mul => *tv *= av,
                Binop::Div => {
                    if av == 0 {
                        return Err(Error::DivideByZero);
                    }
                    *tv /= av;
                }
                Binop::Lt => *top = Val::Bool(*tv < av),
                Binop::Le => *top = Val::Bool(*tv <= av),
                Binop::Gt => *top = Val::Bool(*tv > av),
                Binop::Ge => *top = Val::Bool(*tv >= av),
                Binop::Eq | Binop::NotEq => unreachable!(),
            }
        }
    }
    Ok(())
}

/// Register and return a recursive Fibonacci function.
///
/// Implements: `fib(n) = if n < 2 { n } else { fib(n-1) + fib(n-2) }`
pub fn make_fib() -> Val {
    // Stack on entry: [n]
    let load_n = Instruction::LoadLocal(0);
    Func::new(
        1,
        vec![
            load_n,                         //  0: [n, n]
            Instruction::LessThan(2),       //  1: [n, n<2]
            Instruction::JumpIf(8),         //  2: [n]  -- if n<2 jump to 11
            load_n,                         //  3: [n, n]
            Instruction::AddN(-1),          //  4: [n, n-1]
            Instruction::Eval(-127),        //  5: [n, fib(n-1)]
            load_n,                         //  6: [n, fib(n-1), n]
            Instruction::AddN(-2),          //  7: [n, fib(n-1), n-2]
            Instruction::Eval(-127),        //  8: [n, fib(n-1), fib(n-2)]
            Instruction::Binop(Binop::Add), //  9: [n, fib(n-1)+fib(n-2)]
            Instruction::Return,            // 10: return fib(n-1)+fib(n-2)
            load_n,                         // 11: [n, n]  -- base case (n < 2)
            Instruction::Return,            // 12: return n
        ],
        vec![],
    )
    .into()
}

pub fn make_adder() -> Val {
    Func::new(
        4,
        vec![
            Instruction::LoadLocal(0),
            Instruction::LoadLocal(1),
            Instruction::Binop(Binop::Add),
            Instruction::LoadLocal(2),
            Instruction::LoadLocal(3),
            Instruction::Binop(Binop::Add),
            Instruction::Binop(Binop::Add),
            Instruction::Return,
        ],
        vec![],
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn call_fib(n: i64) -> Result<Val> {
        let mut vm = Vm::new();
        let fib = make_fib();
        vm.run_with_func(fib, [Val::Int(n)])
    }

    #[test]
    fn fib_with_base_case_returns_n() {
        assert_eq!(call_fib(0), Ok(Val::Int(0)));
        assert_eq!(call_fib(1), Ok(Val::Int(1)));
    }

    #[test]
    fn fib_with_recursive_case_returns_sum() {
        assert_eq!(call_fib(2), Ok(Val::Int(1)));
        assert_eq!(call_fib(3), Ok(Val::Int(2)));
        assert_eq!(call_fib(4), Ok(Val::Int(3)));
        assert_eq!(call_fib(5), Ok(Val::Int(5)));
        assert_eq!(call_fib(6), Ok(Val::Int(8)));
    }

    #[test]
    fn string_length_with_valid_string_returns_length() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadConst(0),
                Instruction::StringLength,
                Instruction::Return,
            ],
            vec!["hello".into()],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(5)));
    }

    #[test]
    fn compare_literal_with_various_ops_returns_bool() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(10),
                Instruction::GreaterThan(5),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Bool(true)));

        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(10),
                Instruction::Equal(10),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Bool(true)));

        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(10),
                Instruction::Equal(5),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Bool(false)));
    }

    #[test]
    fn forward_jump_with_jump_if_skips_instructions() {
        let mut vm = Vm::new();

        // Test JumpIf (simulating JumpIfEq): jump if 5 == 5
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(5),
                Instruction::Equal(5),
                Instruction::JumpIf(1),
                Instruction::Return, // Should be skipped
                Instruction::LoadInt(10),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(10)));

        // Test JumpIf (simulating JumpIfLt): jump if 3 < 5
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(3),
                Instruction::LessThan(5),
                Instruction::JumpIf(1),
                Instruction::Return, // Should be skipped
                Instruction::LoadInt(20),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(20)));

        // Test JumpIf (simulating JumpIfGt): no jump if 3 > 5
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(3),
                Instruction::GreaterThan(5),
                Instruction::JumpIf(1),
                Instruction::LoadInt(30),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(30)));
    }

    #[test]
    fn backward_jump_with_jump_implements_loop() {
        let mut vm = Vm::new();
        // Simulate a for-loop: count down from 5 to 0
        // for (i=5; i>0; i--) {} return i;
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(5),     // 0: [5]
                Instruction::LoadLocal(0),   // 1: [5, 5] <-- target
                Instruction::GreaterThan(0), // 2: [5, true]
                Instruction::JumpIf(1),      // 3: [5] -> if true, jump to index 5
                Instruction::Return,         // 4: if false, return [0]
                Instruction::AddN(-1),       // 5: [4]
                Instruction::Jump(-6),       // 6: jump to index 1 (6+1-6=1)
            ],
            vec![],
        );

        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(0)));
    }

    #[test]
    fn set_local_with_new_value_updates_stack() {
        let mut vm = Vm::new();
        let func = Func::new(
            1,
            vec![
                Instruction::LoadInt(10),
                Instruction::SetLocal(0),
                Instruction::LoadLocal(0),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(
            vm.run_with_func(func.into(), [Val::Int(5)]),
            Ok(Val::Int(10))
        );
    }

    #[test]
    fn divide_by_zero_returns_error() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(10),
                Instruction::LoadInt(0),
                Instruction::Binop(Binop::Div),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Err(Error::DivideByZero));
    }

    #[test]
    fn wrong_arg_count_returns_error() {
        let mut vm = Vm::new();
        let func = Func::new(1, vec![Instruction::Return], vec![]);
        // Expected 1, got 0
        assert_eq!(
            vm.run_with_func(func.into(), []),
            Err(Error::WrongArgCount {
                expected: 1,
                got: 0
            })
        );
    }

    #[test]
    fn binop_with_wrong_type_returns_error() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadConst(0),
                Instruction::LoadInt(10),
                Instruction::Binop(Binop::Add),
                Instruction::Return,
            ],
            vec!["not an int".into()],
        );
        assert_eq!(
            vm.run_with_func(func.into(), []),
            Err(Error::WrongType {
                expected: "Int",
                got: "Str"
            })
        );
    }

    #[test]
    fn stack_underflow_returns_error() {
        let mut vm = Vm::new();
        // Binop requires 2 values, but we only push 1.
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(10),
                Instruction::Binop(Binop::Add),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(
            vm.run_with_func(func.into(), []),
            Err(Error::StackUnderflow)
        );
    }

    #[test]
    fn non_func_value_returns_error() {
        let mut vm = Vm::new();
        let not_a_func = Val::Int(42);
        assert_eq!(
            vm.run_with_func(not_a_func, []),
            Err(Error::WrongType {
                expected: "Func or NativeFunc",
                got: "Int"
            })
        );
    }
}

#[cfg(test)]
mod new_tests {
    use super::*;

    #[test]
    fn binop_sub_returns_difference() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(10),
                Instruction::LoadInt(3),
                Instruction::Binop(Binop::Sub),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(7)));
    }

    #[test]
    fn binop_mul_returns_product() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(4),
                Instruction::LoadInt(5),
                Instruction::Binop(Binop::Mul),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(20)));
    }

    #[test]
    fn binop_div_returns_quotient() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(20),
                Instruction::LoadInt(4),
                Instruction::Binop(Binop::Div),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(5)));
    }

    #[test]
    fn binop_comparison_returns_bool() {
        let cases: &[(Binop, i8, i8, bool)] = &[
            (Binop::Lt, 3, 5, true),
            (Binop::Lt, 5, 3, false),
            (Binop::Le, 3, 5, true),
            (Binop::Le, 5, 5, true),
            (Binop::Le, 6, 5, false),
            (Binop::Gt, 5, 3, true),
            (Binop::Gt, 3, 5, false),
            (Binop::Ge, 5, 3, true),
            (Binop::Ge, 5, 5, true),
            (Binop::Ge, 3, 5, false),
        ];
        for &(op, lhs, rhs, expected) in cases {
            let mut vm = Vm::new();
            let func = Func::new(
                0,
                vec![
                    Instruction::LoadInt(lhs),
                    Instruction::LoadInt(rhs),
                    Instruction::Binop(op),
                    Instruction::Return,
                ],
                vec![],
            );
            assert_eq!(
                vm.run_with_func(func.into(), []),
                Ok(Val::Bool(expected)),
                "op={op:?} lhs={lhs} rhs={rhs}"
            );
        }
    }

    #[test]
    fn binop_eq_with_same_values_returns_true() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(5),
                Instruction::LoadInt(5),
                Instruction::Binop(Binop::Eq),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Bool(true)));
    }

    #[test]
    fn binop_not_eq_with_different_values_returns_true() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(3),
                Instruction::LoadInt(5),
                Instruction::Binop(Binop::NotEq),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Bool(true)));
    }

    #[test]
    fn binop_eq_with_different_types_returns_false() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadConst(0),
                Instruction::LoadConst(1),
                Instruction::Binop(Binop::Eq),
                Instruction::Return,
            ],
            vec![true.into(), Val::Int(1)],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Bool(false)));
    }

    #[test]
    fn forward_jump_skips_instructions() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::Jump(1),     // 0: jump to idx 2
                Instruction::LoadInt(99), // 1: skipped
                Instruction::LoadInt(42), // 2: executed
                Instruction::Return,      // 3
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(42)));
    }

    #[test]
    fn backward_jump_if_implements_loop() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(3),     // 0
                Instruction::AddN(-1),       // 1  <- loop target
                Instruction::LoadLocal(0),   // 2
                Instruction::GreaterThan(0), // 3
                Instruction::JumpIf(-4),     // 4: if true -> idx 1
                Instruction::Return,         // 5
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(0)));
    }

    #[test]
    fn load_int_with_negative_value_pushes_correctly() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![Instruction::LoadInt(-5), Instruction::Return],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(-5)));
    }

    #[test]
    fn string_length_with_symbol_returns_length() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadConst(0),
                Instruction::StringLength,
                Instruction::Return,
            ],
            vec![Symbol::new("hi").unwrap().into()],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(2)));
    }

    #[test]
    fn native_func_called_via_eval_returns_result() {
        use crate::val::NativeFunc;
        fn double(args: &[Val]) -> Result<Val> {
            let Val::Int(n) = args[0] else {
                panic!("expected Int")
            };
            Ok(Val::Int(n * 2))
        }
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(5),   // 0: push arg
                Instruction::LoadConst(0), // 1: push NativeFunc
                Instruction::Eval(1),      // 2: call with 1 arg
                Instruction::Return,       // 3
            ],
            vec![Val::NativeFunc(NativeFunc(double))],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(10)));
    }

    #[test]
    fn add_n_with_non_int_returns_wrong_type() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadConst(0),
                Instruction::AddN(1),
                Instruction::Return,
            ],
            vec!["x".into()],
        );
        assert_eq!(
            vm.run_with_func(func.into(), []),
            Err(Error::WrongType {
                expected: "Int",
                got: "Str"
            })
        );
    }

    #[test]
    fn less_than_with_non_int_returns_wrong_type() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadConst(0),
                Instruction::LessThan(5),
                Instruction::Return,
            ],
            vec!["x".into()],
        );
        assert_eq!(
            vm.run_with_func(func.into(), []),
            Err(Error::WrongType {
                expected: "Int",
                got: "Str"
            })
        );
    }

    #[test]
    fn greater_than_with_non_int_returns_wrong_type() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadConst(0),
                Instruction::GreaterThan(5),
                Instruction::Return,
            ],
            vec!["x".into()],
        );
        assert_eq!(
            vm.run_with_func(func.into(), []),
            Err(Error::WrongType {
                expected: "Int",
                got: "Str"
            })
        );
    }

    #[test]
    fn equal_with_non_int_returns_wrong_type() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadConst(0),
                Instruction::Equal(5),
                Instruction::Return,
            ],
            vec!["x".into()],
        );
        assert_eq!(
            vm.run_with_func(func.into(), []),
            Err(Error::WrongType {
                expected: "Int",
                got: "Str"
            })
        );
    }

    #[test]
    fn string_length_with_non_string_returns_wrong_type() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(42),
                Instruction::StringLength,
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(
            vm.run_with_func(func.into(), []),
            Err(Error::WrongType {
                expected: "Str or Symbol",
                got: "Int"
            })
        );
    }

    #[test]
    fn eval_with_non_func_returns_wrong_type() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(5),
                Instruction::Eval(0),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(
            vm.run_with_func(func.into(), []),
            Err(Error::WrongType {
                expected: "Func",
                got: "Int"
            })
        );
    }

    #[test]
    fn eval_with_wrong_arg_count_returns_error() {
        let mut vm = Vm::new();
        let inner_func = Func::new(1, vec![Instruction::Return], vec![]);
        let func = Func::new(
            0,
            vec![
                Instruction::LoadConst(0),
                Instruction::Eval(0),
                Instruction::Return,
            ],
            vec![inner_func.into()],
        );
        assert_eq!(
            vm.run_with_func(func.into(), []),
            Err(Error::WrongArgCount {
                expected: 1,
                got: 0
            })
        );
    }

    #[test]
    fn dup_copies_top_of_stack() {
        let mut vm = Vm::new();
        let func = Func::new(
            0,
            vec![
                Instruction::LoadInt(7),
                Instruction::Dup(1),
                Instruction::Binop(Binop::Add),
                Instruction::Return,
            ],
            vec![],
        );
        assert_eq!(vm.run_with_func(func.into(), []), Ok(Val::Int(14)));
    }
}
