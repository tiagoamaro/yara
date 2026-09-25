module Yara
  class Interpreter
    private

    # `print`, a registry builtin, or a user function, in that order.
    #
    # @param callee [String]
    # @param args [Array<AST::Node>]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Object]
    def call_function(callee, args, line, column)
      canonical = @vocabulary.canonical_builtin(callee)
      if canonical == "print"
        @stdout.puts(args.map { |arg| display(eval_expr(arg)) }.join(" "))
        return nil
      end
      return send("eval_builtin_#{canonical}", args, line, column) if Builtins::ARITIES.key?(canonical)

      function = @functions[callee] || runtime_error_msg("runtime/undefined-function", [callee], line, column)
      values = args.map { |arg| eval_expr(arg) }
      with_frame(callee, line, column) do
        function.params.zip(values).each { |name, value| @environment.declare(name, value) }
        exec_function_body(function.body)
      end
    end

    # @return [Integer]
    def eval_builtin_len(args, line, column)
      array = eval_expr(args[0])
      return array.size if array.is_a?(Array)

      runtime_error_msg("runtime/len-expects-array", [display(array)], line, column)
    end

    # @return [nil]
    def eval_builtin_push(args, line, column)
      array = eval_expr(args[0])
      value = eval_expr(args[1])
      runtime_error_msg("runtime/push-expects-array", [display(array)], line, column) unless array.is_a?(Array)
      array << value
      nil
    end

    # @return [Object]
    def eval_builtin_get(args, line, column)
      array = eval_expr(args[0])
      array_get(array, eval_int(args[1], line, column), line, column)
    end

    # @return [nil]
    def eval_builtin_set(args, line, column)
      array = eval_expr(args[0])
      index = eval_int(args[1], line, column)
      value = eval_expr(args[2])
      runtime_error_msg("runtime/set-expects-array", [display(array)], line, column) unless array.is_a?(Array)
      array_set(array, index, value, line, column)
    end

    # @return [Object]
    def eval_builtin_pop(args, line, column)
      array = eval_expr(args[0])
      runtime_error_msg("runtime/pop-expects-array", [display(array)], line, column) unless array.is_a?(Array)
      array_pop(array, line, column)
    end

    # @return [Pointer]
    def eval_builtin_alloc(args, _line, _column)
      @heap << [eval_expr(args[0])]
      Pointer.new(@heap.size - 1)
    end

    # @return [Object]
    def eval_builtin_deref(args, line, column)
      heap_read(eval_expr(args[0]), line, column)
    end

    # @return [nil]
    def eval_builtin_set_deref(args, line, column)
      pointer = eval_expr(args[0])
      heap_write(pointer, eval_expr(args[1]), line, column)
    end

    # @return [nil]
    def eval_builtin_free(args, line, column)
      heap_free(eval_expr(args[0]), line, column)
    end

    # @return [Integer] how many slots the collector freed
    def eval_builtin_collect(_args, _line, _column)
      collect_garbage
    end

    # Shared by `set` and `Array#set`.
    #
    # @return [nil]
    def array_set(array, index, value, line, column)
      check_bounds(array, index, line, column)
      array[index] = value
      nil
    end

    # Shared by `pop` and `Array#pop`.
    #
    # @return [Object]
    def array_pop(array, line, column)
      runtime_error("cannot `pop` from an empty array", line, column) if array.empty?
      array.pop
    end

    # The live heap slot a pointer names. A slot is a one-element array while
    # allocated and nil once freed; slots are never reused, so a stale
    # pointer always reports an error.
    #
    # @param pointer [Object]
    # @param name [String] the operation, for the non-pointer error key
    # @param line [Integer]
    # @param column [Integer]
    # @return [Array(Object)]
    def heap_slot(pointer, name, line, column)
      unless pointer.is_a?(Pointer)
        runtime_error_msg("runtime/#{name}-expects-pointer", [display(pointer)], line, column)
      end
      runtime_error_msg("runtime/invalid-pointer", [pointer.index.to_s], line, column) if pointer.index >= @heap.size
      @heap[pointer.index] || runtime_error_msg("runtime/use-after-free", [pointer.index.to_s], line, column)
    end

    # @return [Object]
    def heap_read(pointer, line, column)
      runtime_error("nil pointer dereference: `deref` on `nil`", line, column) if pointer.nil?
      heap_slot(pointer, "deref", line, column)[0]
    end

    # @return [nil]
    def heap_write(pointer, value, line, column)
      runtime_error("nil pointer dereference: `set_deref` on `nil`", line, column) if pointer.nil?
      heap_slot(pointer, "set-deref", line, column)[0] = value
      nil
    end

    # @return [nil]
    def heap_free(pointer, line, column)
      runtime_error("cannot `free` a nil pointer", line, column) if pointer.nil?
      if pointer.is_a?(Pointer) && pointer.index < @heap.size && @heap[pointer.index].nil?
        runtime_error_msg("runtime/double-free", [pointer.index.to_s], line, column)
      end
      heap_slot(pointer, "free", line, column)
      @heap[pointer.index] = nil
    end

    # Mark and sweep: every value bound in any scope is a root, reachability
    # follows array elements, instance fields and pointees, and every
    # allocated slot left unmarked is freed.
    #
    # @return [Integer] the number of slots freed
    def collect_garbage
      marked = {}
      seen = {}
      @environment.values.each { |root| mark(root, marked, seen) }
      freed = 0
      @heap.each_index do |index|
        next if @heap[index].nil? || marked[index]

        @heap[index] = nil
        freed += 1
      end
      freed
    end

    # @param value [Object]
    # @param marked [Hash{Integer => true}] heap indexes reached
    # @param seen [Hash{Integer => true}] object ids of containers walked, so cycles end
    # @return [void]
    def mark(value, marked, seen)
      case value
      when Pointer
        return if marked[value.index]

        marked[value.index] = true
        slot = @heap[value.index]
        mark(slot[0], marked, seen) if slot
      when Array, Instance
        return if seen[value.object_id]

        seen[value.object_id] = true
        (value.is_a?(Array) ? value : value.fields.values).each { |element| mark(element, marked, seen) }
      end
    end
  end
end
