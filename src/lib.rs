#![feature(explicit_tail_calls)]
mod func;
mod val;
pub use func::make_fib;
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
    pub(crate) stack_frames: Vec<StackFrame>,
}

#[derive(Debug)]
pub struct StackFrame {
    pub(crate) stack_start: usize,
    pub(crate) instruction_idx: usize,
    pub(crate) func: Func,
}

impl StackFrame {
    #[inline(always)]
    pub(crate) fn advance_instruction_fn(
        &mut self,
    ) -> (fn(&mut Vm, StackFrame, u8) -> Result<Val>, u8) {
        let (func, data) = self.func.instruction_fn(self.instruction_idx);
        self.instruction_idx += 1;
        (func, data)
    }
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
        let (fn_ptr, data) = current_frame.advance_instruction_fn();
        fn_ptr(self, current_frame, data)
    }
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
