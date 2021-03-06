use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::compiler::code::{Instructions, Opcode, read_operands};
use crate::object::{HashKey, Object, RuntimeError};
use crate::object::builtins::BUILTINS;
use crate::vm::{FALSE, NULL, TRUE, Vm, VmResult};
use crate::vm::frame::Frame;

impl Vm {
    pub fn jump_if(&mut self, truthy: bool, ins: &Instructions, ip: usize) {
        if truthy {
            self.frames.last_mut().unwrap().ip += 2;
        } else {
            self.frames.last_mut().unwrap().ip = self.read_u16(ins, ip);
        }
    }
    /// # 执行非运算
    pub fn execute_not_expression(&mut self, value: &Object) -> VmResult<()> {
        if *value == TRUE {
            self.push_stack(self.bool_cache_false.clone());
        } else if *value == FALSE {
            self.push_stack(self.bool_cache_true.clone());
        } else if *value == NULL {
            self.push_stack(self.null_cache.clone());
        } else {
            return Err(RuntimeError::UnSupportedUnOperation(
                Opcode::Not,
                (value).clone(),
            ));
        }
        Ok(())
    }
    /// # 执行赋值操作
    pub fn execute_assign_operation_or_pop_and_set_global(
        &mut self,
        index: usize,
        is_local: bool,
    ) -> VmResult<()> {
        let opt = if is_local {
            self.get_local(index)
        } else {
            self.get_global(index)
        };
        match opt {
            None => {
                //声明
                if is_local {
                    self.pop_and_set_local(index);
                } else {
                    self.pop_and_set_global(index);
                }
            }
            Some(obj) => {
                //赋值
                match obj.as_ref() {
                    //数组赋值
                    Object::Array(items) => {
                        let val = self.pop_stack();
                        let index = self.pop_stack();
                        if let Object::Integer(i) = index.as_ref() {
                            items.borrow_mut()[*i as usize] = Object::clone(&val);
                        } else {
                            return Err(RuntimeError::UnSupportedIndexOperation(
                                Object::clone(&obj),
                                Object::clone(&index),
                            ));
                        }
                    }
                    // // Hash赋值
                    Object::Hash(pairs) => {
                        let val = self.pop_stack();
                        let index = self.pop_stack();
                        let key = HashKey::from_object(&*index)?;
                        pairs.borrow_mut().insert(key, Object::clone(&val));
                    }
                    //普通赋值
                    _ => {
                        if is_local {
                            self.pop_and_set_local(index);
                        } else {
                            self.pop_and_set_global(index);
                        }
                    }
                }
            }
        }
        Ok(())
    }
    /// # 创建数组
    pub fn build_array(&mut self, arr_len: usize) {
        let mut arr = vec![];
        for i in self.sp - arr_len..self.sp {
            let el = &self.stack[i];
            arr.push(Object::clone(el));
        }
        self.sp -= arr_len;
        self.push_stack(Rc::new(Object::Array(RefCell::new(arr))))
    }
    /// # 创建Hash
    pub fn build_hash(&mut self, hash_len: usize) -> VmResult<()> {
        let mut hash = HashMap::new();
        let mut i = self.sp - 2 * hash_len;
        while i < self.sp {
            let k = &self.stack[i];
            let v = &self.stack[i + 1];
            let key = HashKey::from_object(k)?;
            hash.insert(key, Object::clone(v));
            i += 2;
        }
        self.sp -= hash_len;
        self.push_stack(Rc::new(Object::Hash(RefCell::new(hash))));
        Ok(())
    }
    /// # 执行二元操作
    // #[inline]
    pub fn execute_binary_operation(&mut self, op: &Opcode) -> VmResult<()> {
        let right = &*self.pop_stack();
        let left = &*self.pop_stack();
        let result = match (left, right) {
            (Object::Integer(left_val), Object::Integer(right_val)) => {
                let r = match op {
                    Opcode::Add => left_val + right_val,
                    Opcode::Sub => left_val - right_val,
                    Opcode::Mul => left_val * right_val,
                    Opcode::Div => {
                        if right_val == &0 {
                            return Err(RuntimeError::ByZero(
                                Object::clone(&left),
                                Object::clone(&right),
                            ));
                        }
                        left_val / right_val
                    }
                    _ => return Err(RuntimeError::UnSupportedBinOperator(op.clone())),
                };
                match self.int_cache.get(r as usize) {
                    None => Rc::new(Object::Integer(r)),
                    Some(v) => v.clone(),
                }
            }
            (Object::String(left_val), Object::String(right_val)) => {
                if let Opcode::Add = op {
                    Rc::new(Object::String(left_val.clone() + right_val))
                } else {
                    return Err(RuntimeError::UnSupportedBinOperation(
                        op.clone(),
                        left.clone(),
                        right.clone(),
                    ));
                }
            }
            (Object::Integer(left_val), Object::String(right_val)) => {
                if let Opcode::Add = op {
                    Rc::new(Object::String(left_val.to_string() + right_val))
                } else {
                    return Err(RuntimeError::UnSupportedBinOperation(
                        op.clone(),
                        left.clone(),
                        right.clone(),
                    ));
                }
            }
            (Object::String(left_val), Object::Integer(right_val)) => {
                if let Opcode::Add = op {
                    Rc::new(Object::String(left_val.clone() + &right_val.to_string()))
                } else {
                    return Err(RuntimeError::UnSupportedBinOperation(
                        op.clone(),
                        left.clone(),
                        right.clone(),
                    ));
                }
            }
            _ => {
                return Err(RuntimeError::UnSupportedBinOperation(
                    op.clone(),
                    left.clone(),
                    right.clone(),
                ))
            }
        };
        self.push_stack(result);
        Ok(())
    }
    /// # 执行索引操作
    pub fn execute_index_operation(&self, obj: &Object, index: &Object) -> VmResult {
        if let Object::Array(items) = obj {
            if let Object::Integer(index) = index {
                let value = items.borrow().get(*index as usize).cloned().unwrap_or(NULL);
                return Ok(Rc::new(value));
            }
        } else if let Object::Hash(pairs) = obj {
            let key = HashKey::from_object(index)?;
            let value = pairs.borrow().get(&key).cloned().unwrap_or(NULL);
            return Ok(Rc::new(value));
        }
        Err(RuntimeError::UnSupportedIndexOperation(
            obj.clone(),
            index.clone(),
        ))
    }
    /// # 执行比较操作
    pub fn execute_comparison_operation(&mut self, op: &Opcode) -> VmResult {
        let right = self.pop_stack();
        let left = self.pop_stack();
        if let (Object::Integer(left), Object::Integer(right)) = (left.as_ref(), right.as_ref()) {
            let bool = match op {
                Opcode::GreaterThan => left > right,
                Opcode::GreaterEq => left >= right,
                Opcode::LessThan => left < right,
                Opcode::LessEq => left <= right,
                Opcode::Equal => left == right,
                Opcode::NotEqual => left != right,
                _ => return Err(RuntimeError::UnSupportedBinOperator(op.clone())),
            };
            Ok(self.get_bool_from_cache(bool))
        } else {
            match op {
                Opcode::Equal => Ok(self.get_bool_from_cache(left == right)),
                Opcode::NotEqual => Ok(self.get_bool_from_cache(left != right)),
                _ => Err(RuntimeError::UnSupportedBinOperation(
                    op.clone(),
                    Object::clone(&left),
                    Object::clone(&right),
                )),
            }
        }
    }
    pub fn get_bool_from_cache(&self, bool: bool) -> Rc<Object> {
        if bool {
            self.bool_cache_true.clone()
        } else {
            self.bool_cache_false.clone()
        }
    }
    /// 函数调用
    #[inline]
    pub fn call_function(&mut self, arg_nums: usize) -> VmResult<()> {
        self.sp -= arg_nums;
        let callee = &self.stack[self.sp - 1]; //往回跳过参数个数位置, 当前位置是函数
        match callee.as_ref() {
            Object::Closure(closure) => {
                if arg_nums != closure.compiled_function.num_parameters {
                    return Err(RuntimeError::WrongArgumentCount(
                        closure.compiled_function.num_parameters,
                        arg_nums,
                    ));
                }
                // let num_locals = closure.compiled_function.num_locals;
                let frame = Frame::new(callee.clone(), self.sp);
                // Equivalent to
                self.sp += closure.compiled_function.num_locals;
                // self.sp = frame.base_pointer + num_locals;
                self.push_frame(frame); //进入函数内部（下一帧）
            }
            Object::Builtin(builtin_fun) => {
                //内置函数
                let mut v = vec![];
                for i in 0..arg_nums {
                    let rc = &self.stack[self.sp + i];
                    v.push(Object::clone(rc));
                }
                let r = builtin_fun(v)?;
                self.sp -= 1;
                self.push_stack(Rc::new(r));
            }
            _ => {
                return Err(RuntimeError::CustomErrMsg(
                    "calling non-function".to_string(),
                ))
            }
        }
        // self.pop_stack();
        Ok(())
    }
    /// # 读取一个无符号整数，并返回字节长度

    pub fn read_usize(&self, op_code: Opcode, ip: usize) -> (usize, usize) {
        let (operands, n) = read_operands(
            &op_code.definition(),
            &self.current_frame().instructions()[ip..],
        );
        (operands[0], n)
    }
    #[inline]
    pub fn read_u16(&self, insts: &[u8], start: usize) -> usize {
        u16::from_be_bytes([insts[start], insts[start + 1]]) as usize
    }
    // #[inline]
    pub fn _read_u8(&self, insts: &[u8], start: usize) -> usize {
        insts[start] as usize
    }
    /// # 压入栈中
    pub fn push_stack(&mut self, object: Rc<Object>) {
        if self.sp == self.stack.len() {
            self.stack.push(object);
        } else {
            self.stack[self.sp] = object;
        }
        self.sp += 1;
    }
    /// # 弹出栈顶元素
    // #[inline]
    pub fn pop_stack(&mut self) -> Rc<Object> {
        if self.sp > 0 {
            self.sp -= 1;
            self.stack[self.sp].clone()
        } else {
            self.null_cache.clone()
        }
    }

    /// # 栈顶元素存入全局变量
    pub fn pop_and_set_global(&mut self, global_index: usize) {
        let global = self.pop_stack();
        if global_index >= self.globals.borrow().len() {
            self.globals.borrow_mut().push(global);
        } else {
            self.globals.borrow_mut()[global_index] = global;
        }
    }
    /// # 弹出栈顶元素并设置到指定位置
    pub fn pop_and_set_local(&mut self, local_index: usize) {
        let popped = self.pop_stack();
        let frame = self.frames.last().unwrap();
        self.stack[frame.base_pointer + local_index] = popped;
    }
    /// # 取出全局变量
    pub fn get_global(&self, global_index: usize) -> Option<Rc<Object>> {
        self.globals.borrow().get(global_index).cloned()
    }
    pub fn get_local(&mut self, local_index: usize) -> Option<Rc<Object>> {
        let frame = self.frames.last().unwrap();
        self.stack.get(frame.base_pointer + local_index).cloned()
    }
    /// # 取出局部变量并压入栈顶
    pub fn get_local_and_push(&mut self, local_index: usize) {
        let object = self.get_local(local_index).unwrap();
        self.push_stack(object)
    }
    /// # 取出全局变量并压入栈顶
    pub fn get_global_and_push(&mut self, global_index: usize) {
        let object = self.get_global(global_index).unwrap();
        self.push_stack(object)
    }
    pub fn get_builtin(&self, builtin_index: usize) -> VmResult {
        let builtin_fun = &BUILTINS[builtin_index];
        Ok(Rc::new(builtin_fun.builtin.clone()))
    }
    /// # 最后弹出栈顶的元素
    pub fn last_popped_stack_element(&self) -> VmResult {
        let object = &self.stack[self.sp];
        Ok(object.clone())
    }
    pub fn get_const_object(&self, index: usize) -> Rc<Object> {
        self.constants[index].clone()
    }
    pub fn current_frame_ip_inc(&mut self, n: usize) {
        self.frames.last_mut().unwrap().ip += n;
    }
    pub fn current_frame(&self) -> &Frame {
        &self.frames.last().unwrap()
    }
    pub fn push_frame(&mut self, frame: Frame) {
        self.frames.push(frame);
    }
    pub fn pop_frame(&mut self) -> Frame {
        self.frames.pop().unwrap()
    }
}
