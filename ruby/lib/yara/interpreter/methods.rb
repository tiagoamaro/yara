module Yara
  class Interpreter
    private

    # @param value [Object]
    # @return [Symbol, nil] the primitive-method receiver kind, nil for `nil`
    def receiver_kind(value)
      case value
      when Integer then :integer
      when Float then :float
      when true, false then :boolean
      when String then :string
      when Array then :array
      when Pointer then :pointer
      end
    end

    # @param kind [Symbol]
    # @param receiver [Object]
    # @param method [String] as written, possibly localized
    # @param args [Array<AST::Node>]
    # @param line [Integer]
    # @param column [Integer]
    # @return [Object]
    def eval_primitive_method(kind, receiver, method, args, line, column)
      name = @vocabulary.canonical_method(method)
      unless Methods::ARITIES.key?([kind, name])
        runtime_error_msg("runtime/no-method-for-value", [method], line, column)
      end
      send("eval_#{kind}_#{name}", receiver, args, line, column)
    end

    # @return [Integer]
    def eval_array_size(array, _args, _line, _column)
      array.size
    end

    # @return [nil]
    def eval_array_push(array, args, _line, _column)
      array << eval_expr(args[0])
      nil
    end

    # @return [Object]
    def eval_array_get(array, args, line, column)
      array_get(array, eval_int(args[0], line, column), line, column)
    end

    # @return [nil]
    def eval_array_set(array, args, line, column)
      index = eval_int(args[0], line, column)
      array_set(array, index, eval_expr(args[1]), line, column)
    end

    # @return [Object]
    def eval_array_pop(array, _args, line, column)
      array_pop(array, line, column)
    end

    # @return [Boolean]
    def eval_array_is_empty(array, _args, _line, _column)
      array.empty?
    end

    # @return [Integer] the length in characters
    def eval_string_size(string, _args, _line, _column)
      string.size
    end

    # ponytail: upcase/downcase/strip follow the runtime's own Unicode rules,
    # which match Rust for ASCII (see the parity traps in PLAN.md).
    #
    # @return [String]
    def eval_string_upper(string, _args, _line, _column)
      string.upcase
    end

    # @return [String]
    def eval_string_lower(string, _args, _line, _column)
      string.downcase
    end

    # @return [String]
    def eval_string_trim(string, _args, _line, _column)
      string.strip
    end

    # @return [Boolean]
    def eval_string_is_empty(string, _args, _line, _column)
      string.empty?
    end

    # Rust's `str::parse::<i64>` on the trimmed text: an optional sign and
    # decimal digits only, within the i64 range.
    #
    # @return [Integer]
    def eval_string_to_i(string, _args, line, column)
      text = string.strip
      if digits?(text.start_with?("+", "-") ? text[1..] : text)
        value = text.to_i
        return value if value >= INTEGER_MIN && value <= INTEGER_MAX
      end
      runtime_error_msg("runtime/cannot-parse-as-integer", [string], line, column)
    end

    # Rust's `str::parse::<f64>` on the trimmed text: decimal digits with an
    # optional point and exponent, or `inf`/`infinity`/`nan` in any case.
    #
    # @return [Float]
    def eval_string_to_f(string, _args, line, column)
      text = string.strip
      sign = text.start_with?("-") ? -1.0 : 1.0
      body = text.start_with?("+", "-") ? text[1..] : text
      case body.downcase
      when "inf", "infinity" then return sign * Float::INFINITY
      when "nan" then return Float::NAN
      end
      decimal = decimal_float(body)
      return sign * decimal.to_f if decimal

      runtime_error_msg("runtime/cannot-parse-as-float", [string], line, column)
    end

    # Digits with an optional point and exponent, respelled so `String#to_f`
    # reads every form Rust accepts (`5.`, `.5`, `1.e5`).
    #
    # @param text [String] without a sign
    # @return [String, nil] nil when `text` is not such a number
    def decimal_float(text)
      mantissa, exponent = text.downcase.split("e", 2)
      return nil if mantissa.nil? || mantissa.delete(".").empty?

      whole, fraction = mantissa.split(".", 2)
      whole = "0" if whole.empty?
      fraction = "0" if fraction.nil? || fraction.empty?
      return nil unless digits?(whole) && digits?(fraction)
      return "#{whole}.#{fraction}" if exponent.nil?

      exponent_digits = exponent.start_with?("+", "-") ? exponent[1..] : exponent
      digits?(exponent_digits) ? "#{whole}.#{fraction}e#{exponent}" : nil
    end

    # @param text [String]
    # @return [Boolean] whether `text` is one or more ASCII digits
    def digits?(text)
      !text.empty? && text.each_char.all? { |char| char >= "0" && char <= "9" }
    end

    # @return [String]
    def eval_string_to_s(string, _args, _line, _column)
      string
    end

    # @return [String]
    def eval_integer_to_s(integer, _args, _line, _column)
      integer.to_s
    end

    # @return [Float]
    def eval_integer_to_f(integer, _args, _line, _column)
      integer.to_f
    end

    # @return [Integer]
    def eval_integer_abs(integer, _args, line, column)
      checked_integer(integer.abs, "abs", line, column)
    end

    # @return [String]
    def eval_float_to_s(float, _args, _line, _column)
      RustFormat.float_display(float)
    end

    # Rust's `as i64`: truncates toward zero, saturates at the i64 bounds,
    # and turns NaN into 0.
    #
    # @return [Integer]
    def eval_float_to_i(float, _args, _line, _column)
      return 0 if float.nan?
      return INTEGER_MAX if float >= 2.0**63
      return INTEGER_MIN if float <= -(2.0**63)

      float.truncate
    end

    # @return [Float]
    def eval_float_abs(float, _args, _line, _column)
      float.abs
    end

    # @return [String]
    def eval_boolean_to_s(boolean, _args, _line, _column)
      @vocabulary.bool_word(boolean)
    end

    # @return [Object]
    def eval_pointer_deref(pointer, _args, line, column)
      heap_read(pointer, line, column)
    end

    # @return [nil]
    def eval_pointer_set_deref(pointer, args, line, column)
      heap_write(pointer, eval_expr(args[0]), line, column)
    end

    # @return [nil]
    def eval_pointer_free(pointer, _args, line, column)
      heap_free(pointer, line, column)
    end
  end
end
