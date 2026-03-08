use std::rc::Rc;

use crate::{Error, Result, StackFrame, Val, Vm};

/// A reference-counted function containing bytecode and constants.
#[derive(Clone, Debug, PartialEq)]
pub struct Func(Rc<FuncInner>);

impl Func {
    /// Create a new function with the given argument count, instructions, and constants.
    pub fn new(
        arg_count: usize,
        instructions: impl IntoIterator<Item = Instruction>,
        constants: Vec<Val>,
    ) -> Func {
        let mut funcs = Vec::new();
        let mut data = Vec::new();
        for instruction in instructions {
            let (func, d) = match instruction {
                Instruction::Eval(n) => (eval_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>, n),
                Instruction::EvalRecursive(n) => (
                    eval_recursive_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>,
                    n,
                ),
                Instruction::LoadInt(x) => (
                    load_int_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>,
                    x as u8,
                ),
                Instruction::LoadConst(idx) => (
                    load_const_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>,
                    idx,
                ),
                Instruction::LoadLocal(idx) => (
                    load_local_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>,
                    idx,
                ),
                Instruction::SetLocal(idx) => (
                    set_local_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>,
                    idx,
                ),
                Instruction::JumpIf(n) => (
                    jump_if_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>,
                    n as u8,
                ),
                Instruction::Jump(n) => (
                    jump_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>,
                    n as u8,
                ),
                Instruction::Return => (return_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>, 0),
                Instruction::Binop(op) => {
                    let f: fn(&mut Vm, StackFrame, u8) -> Result<Val> = match op {
                        Binop::Add => binop_add_fn,
                        Binop::Sub => binop_sub_fn,
                        Binop::Mul => binop_mul_fn,
                        Binop::Div => binop_div_fn,
                        Binop::Eq => binop_eq_fn,
                        Binop::NotEq => binop_not_eq_fn,
                        Binop::Lt => binop_lt_fn,
                        Binop::Le => binop_le_fn,
                        Binop::Gt => binop_gt_fn,
                        Binop::Ge => binop_ge_fn,
                    };
                    (f, 0)
                }
                Instruction::AddN(n) => (add_n_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>, n as u8),
                Instruction::LessThan(n) => (less_than_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>, n as u8),
                Instruction::GreaterThan(n) => (
                    greater_than_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>,
                    n as u8,
                ),
                Instruction::Equal(n) => (
                    equal_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>,
                    n as u8,
                ),
                Instruction::StringLength => (
                    string_length_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>,
                    0,
                ),
                Instruction::Dup(n) => (dup_fn as fn(&mut Vm, StackFrame, u8) -> Result<Val>, n),
            };
            funcs.push(func);
            data.push(d);
        }
        Func(Rc::new(FuncInner {
            arg_count,
            funcs,
            data,
            constants,
        }))
    }

    /// The number of arguments the function expects.
    pub fn args(&self) -> usize {
        self.0.arg_count
    }

    /// Get the instruction at the given index.
    pub(crate) fn instruction_fn(
        &self,
        idx: usize,
    ) -> (fn(&mut Vm, StackFrame, u8) -> Result<Val>, u8) {
        (self.0.funcs[idx], self.0.data[idx])
    }

    /// Constant values referenced by `LoadConst` instructions.
    pub fn constants(&self) -> &[Val] {
        &self.0.constants
    }
}

#[derive(Debug, PartialEq)]
struct FuncInner {
    arg_count: usize,
    funcs: Vec<fn(&mut Vm, StackFrame, u8) -> Result<Val>>,
    data: Vec<u8>,
    constants: Vec<Val>,
}

/// A binary operation.
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum Binop {
    /// Integer addition.
    Add,
    /// Integer subtraction.
    Sub,
    /// Integer multiplication.
    Mul,
    /// Integer division.
    Div,
    /// Equality.
    Eq,
    /// Inequality.
    NotEq,
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
}

/// A single VM instruction.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Instruction {
    /// Pop a `Func` from the stack and call it with `n` arguments.
    Eval(u8),
    /// Recursively call the current function with `n` arguments.
    EvalRecursive(u8),
    /// Push a small integer literal onto the stack.
    LoadInt(i8),
    /// Push a constant from the function's constant table onto the stack.
    LoadConst(u8),
    /// Push a local variable (by index into the current stack frame) onto the stack.
    LoadLocal(u8),
    /// Set a local variable (by index into the current stack frame) from the top of the stack.
    SetLocal(u8),
    /// Skip the next `n` instructions if the top of the stack is truthy.
    JumpIf(i8),
    /// Skip the next `n` instructions unconditionally.
    Jump(i8),
    /// Return from the current function.
    Return,
    // The following instructions are "fast-path" operations. They combine
    // common sequences of instructions (like loading a constant and then
    // performing an operation) into a single instruction to improve performance.
    /// Apply a binary operation to the top two stack values, leaving the result.
    Binop(Binop),
    /// Add a small integer literal to the top-of-stack integer in place.
    AddN(i8),
    /// Replace the top-of-stack integer with a bool: `top < n`.
    LessThan(i8),
    /// Replace the top-of-stack integer with a bool: `top > n`.
    GreaterThan(i8),
    /// Replace the top-of-stack integer with a bool: `top == n`.
    Equal(i8),
    /// Replace the top-of-stack string with its integer length.
    StringLength,
    /// Duplicate the top-of-stack value.
    Dup(u8),
}

pub(crate) fn eval_fn(vm: &mut Vm, mut frame: StackFrame, data: u8) -> Result<Val> {
    let func_val = vm.stack.pop().ok_or(Error::StackUnderflow)?;
    let arg_count = data as usize;
    let stack_start = vm.stack.len() - arg_count;
    match func_val {
        Val::Func(func) => {
            if func.args() != arg_count {
                return Err(Error::WrongArgCount {
                    expected: func.args(),
                    got: arg_count,
                });
            }
            vm.stack_frames.push(frame);
            let mut new_frame = StackFrame {
                stack_start,
                instruction_idx: 0,
                func,
            };
            let (fn_ptr, data) = new_frame.advance_instruction_fn();
            become fn_ptr(vm, new_frame, data);
        }
        Val::NativeFunc(nf) => {
            if vm.stack.len() < arg_count {
                return Err(Error::StackUnderflow);
            }
            let result = (nf.0)(&vm.stack[stack_start..])?;
            vm.stack.truncate(stack_start);
            vm.stack.push(result);
            let (fn_ptr, data) = frame.advance_instruction_fn();
            become fn_ptr(vm, frame, data);
        }
        v => {
            return Err(Error::WrongType {
                expected: "Func",
                got: v.type_name(),
            });
        }
    }
}

pub(crate) fn eval_recursive_fn(vm: &mut Vm, frame: StackFrame, data: u8) -> Result<Val> {
    let arg_count = data as usize;
    let func = frame.func.clone();
    let stack_start = vm.stack.len() - arg_count;
    if func.args() != arg_count {
        return Err(Error::WrongArgCount {
            expected: func.args(),
            got: arg_count,
        });
    }
    vm.stack_frames.push(frame);
    let mut new_frame = StackFrame {
        stack_start,
        instruction_idx: 0,
        func,
    };
    let (fn_ptr, data) = new_frame.advance_instruction_fn();
    become fn_ptr(vm, new_frame, data);
}

pub(crate) fn load_int_fn(vm: &mut Vm, mut frame: StackFrame, data: u8) -> Result<Val> {
    vm.stack.push((data as i8 as i64).into());
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn load_const_fn(vm: &mut Vm, mut frame: StackFrame, data: u8) -> Result<Val> {
    let val = frame.func.constants()[data as usize].clone();
    vm.stack.push(val);
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn load_local_fn(vm: &mut Vm, mut frame: StackFrame, data: u8) -> Result<Val> {
    let val = vm.stack[frame.stack_start + data as usize].clone();
    vm.stack.push(val);
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn set_local_fn(vm: &mut Vm, mut frame: StackFrame, data: u8) -> Result<Val> {
    let val = vm.stack.pop().ok_or(Error::StackUnderflow)?;
    vm.stack[frame.stack_start + data as usize] = val;
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn jump_if_fn(vm: &mut Vm, mut frame: StackFrame, data: u8) -> Result<Val> {
    let is_truthy = vm.stack.pop().ok_or(Error::StackUnderflow)?.is_truthy();
    if is_truthy {
        frame.instruction_idx = (frame.instruction_idx as isize + data as i8 as isize) as usize;
    }
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn jump_fn(_vm: &mut Vm, mut frame: StackFrame, data: u8) -> Result<Val> {
    frame.instruction_idx = (frame.instruction_idx as isize + data as i8 as isize) as usize;
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(_vm, frame, data);
}

pub(crate) fn return_fn(vm: &mut Vm, frame: StackFrame, _data: u8) -> Result<Val> {
    let stack_start = frame.stack_start;
    let last = vm.stack.len().checked_sub(1).ok_or(Error::StackUnderflow)?;
    match vm.stack_frames.pop() {
        None => {
            let val = vm.stack.pop().ok_or(Error::StackUnderflow)?;
            Ok(val)
        }
        Some(mut prev_frame) => {
            vm.stack.swap(stack_start, last);
            vm.stack.truncate(stack_start + 1);
            let (fn_ptr, data) = prev_frame.advance_instruction_fn();
            become fn_ptr(vm, prev_frame, data);
        }
    }
}

pub(crate) fn binop_add_fn(vm: &mut Vm, mut frame: StackFrame, _data: u8) -> Result<Val> {
    let a = vm.stack.pop().ok_or(Error::StackUnderflow)?;
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
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
    *tv += av;
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn binop_sub_fn(vm: &mut Vm, mut frame: StackFrame, _data: u8) -> Result<Val> {
    let a = vm.stack.pop().ok_or(Error::StackUnderflow)?;
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
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
    *tv -= av;
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn binop_mul_fn(vm: &mut Vm, mut frame: StackFrame, _data: u8) -> Result<Val> {
    let a = vm.stack.pop().ok_or(Error::StackUnderflow)?;
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
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
    *tv *= av;
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn binop_div_fn(vm: &mut Vm, mut frame: StackFrame, _data: u8) -> Result<Val> {
    let a = vm.stack.pop().ok_or(Error::StackUnderflow)?;
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
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
    if av == 0 {
        return Err(Error::DivideByZero);
    }
    *tv /= av;
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn binop_eq_fn(vm: &mut Vm, mut frame: StackFrame, _data: u8) -> Result<Val> {
    let a = vm.stack.pop().ok_or(Error::StackUnderflow)?;
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
    *top = Val::Bool(a == *top);
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn binop_not_eq_fn(vm: &mut Vm, mut frame: StackFrame, _data: u8) -> Result<Val> {
    let a = vm.stack.pop().ok_or(Error::StackUnderflow)?;
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
    *top = Val::Bool(a != *top);
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn binop_lt_fn(vm: &mut Vm, mut frame: StackFrame, _data: u8) -> Result<Val> {
    let a = vm.stack.pop().ok_or(Error::StackUnderflow)?;
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
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
    *top = Val::Bool(*tv < av);
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn binop_le_fn(vm: &mut Vm, mut frame: StackFrame, _data: u8) -> Result<Val> {
    let a = vm.stack.pop().ok_or(Error::StackUnderflow)?;
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
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
    *top = Val::Bool(*tv <= av);
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn binop_gt_fn(vm: &mut Vm, mut frame: StackFrame, _data: u8) -> Result<Val> {
    let a = vm.stack.pop().ok_or(Error::StackUnderflow)?;
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
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
    *top = Val::Bool(*tv > av);
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn binop_ge_fn(vm: &mut Vm, mut frame: StackFrame, _data: u8) -> Result<Val> {
    let a = vm.stack.pop().ok_or(Error::StackUnderflow)?;
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
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
    *top = Val::Bool(*tv >= av);
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn add_n_fn(vm: &mut Vm, mut frame: StackFrame, data: u8) -> Result<Val> {
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
    match top {
        Val::Int(x) => *x += data as i8 as i64,
        _ => {
            return Err(Error::WrongType {
                expected: "Int",
                got: top.type_name(),
            });
        }
    }
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn less_than_fn(vm: &mut Vm, mut frame: StackFrame, data: u8) -> Result<Val> {
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
    match top {
        Val::Int(x) => {
            let result = *x < data as i8 as i64;
            *top = result.into();
        }
        _ => {
            return Err(Error::WrongType {
                expected: "Int",
                got: top.type_name(),
            });
        }
    }
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn greater_than_fn(vm: &mut Vm, mut frame: StackFrame, data: u8) -> Result<Val> {
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
    match top {
        Val::Int(x) => {
            let result = *x > data as i8 as i64;
            *top = result.into();
        }
        _ => {
            return Err(Error::WrongType {
                expected: "Int",
                got: top.type_name(),
            });
        }
    }
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn equal_fn(vm: &mut Vm, mut frame: StackFrame, data: u8) -> Result<Val> {
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
    match top {
        Val::Int(x) => {
            let result = *x == data as i8 as i64;
            *top = result.into();
        }
        _ => {
            return Err(Error::WrongType {
                expected: "Int",
                got: top.type_name(),
            });
        }
    }
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn string_length_fn(vm: &mut Vm, mut frame: StackFrame, _data: u8) -> Result<Val> {
    let top = vm.stack.last_mut().ok_or(Error::StackUnderflow)?;
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
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
}

pub(crate) fn dup_fn(vm: &mut Vm, mut frame: StackFrame, data: u8) -> Result<Val> {
    let val = vm.stack.last().ok_or(Error::StackUnderflow)?.clone();
    vm.stack
        .extend(std::iter::repeat_with(|| val.clone()).take(data as usize));
    let (fn_ptr, data) = frame.advance_instruction_fn();
    become fn_ptr(vm, frame, data);
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
            Instruction::EvalRecursive(1),  //  5: [n, fib(n-1)]
            load_n,                         //  6: [n, fib(n-1), n]
            Instruction::AddN(-2),          //  7: [n, fib(n-1), n-2]
            Instruction::EvalRecursive(1),  //  8: [n, fib(n-1), fib(n-2)]
            Instruction::Binop(Binop::Add), //  9: [n, fib(n-1)+fib(n-2)]
            Instruction::Return,            // 10: return fib(n-1)+fib(n-2)
            load_n,                         // 11: [n, n]  -- base case (n < 2)
            Instruction::Return,            // 12: return n
        ],
        vec![],
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_with_default_state_has_size_two() {
        assert_eq!(std::mem::size_of::<Instruction>(), 2);
    }
}
