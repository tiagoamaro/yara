module Yara
  # Reproduces how the Rust implementation formats floats and strings, since
  # that text reaches stdout and error messages. `Float#to_s` can't be used:
  # it differs from Rust, and CRuby and mruby differ from each other.
  module RustFormat
    # Rust's `{}` for a float: never an exponent, no `.0` on whole numbers
    # (`5`, `0.1`, `100000000000000000000`).
    #
    # @param value [Float]
    # @return [String]
    def self.float_display(value)
      special = special_float(value)
      return special if special
      return (sign(value) + "0") if value.zero?

      digits, exponent = shortest_digits(value)
      sign(value) + positional(digits, exponent)
    end

    # Rust's `{:?}` for a float: always shows a fraction (`5.0`), and switches
    # to exponent form (`1e16`, `1.5e-5`) outside `1e-4 <= |value| < 1e16`.
    #
    # @param value [Float]
    # @return [String]
    def self.float_debug(value)
      special = special_float(value)
      return special if special
      return (sign(value) + "0.0") if value.zero?

      digits, exponent = shortest_digits(value)
      if exponent < -4 || exponent >= 16
        mantissa = digits.size == 1 ? digits : "#{digits[0]}.#{digits[1..]}"
        return "#{sign(value)}#{mantissa}e#{exponent}"
      end

      text = positional(digits, exponent)
      text += ".0" unless text.include?(".")
      sign(value) + text
    end

    # Rust's `{:?}` for a string: double-quoted, with `"`, `\`, tab, newline,
    # carriage return and NUL backslash-escaped and other control characters
    # as `\u{hex}`.
    #
    # @param value [String]
    # @return [String]
    def self.string_debug(value)
      escaped = value.each_char.map do |char|
        case char
        when "\"" then "\\\""
        when "\\" then "\\\\"
        when "\t" then "\\t"
        when "\n" then "\\n"
        when "\r" then "\\r"
        when "\0" then "\\0"
        else
          # ponytail: escapes ASCII controls only; Rust also escapes
          # non-printable Unicode (e.g. U+200B), add a table if one shows up.
          code = char.ord
          code < 0x20 || code == 0x7f ? "\\u{#{code.to_s(16)}}" : char
        end
      end
      "\"#{escaped.join}\""
    end

    # The shortest decimal digits that round-trip to `value`, with the
    # base-10 exponent of the first digit: 0.015 gives `["15", -2]`. For each
    # length, rounds the exact value to that many digits and keeps the first
    # that parses back to `value`, all in exact integer arithmetic, since
    # mruby's `%e` and `String#to_f` are not exact.
    #
    # @param value [Float] finite and nonzero
    # @return [Array(String, Integer)]
    def self.shortest_digits(value)
      magnitude = value.abs
      mantissa, exponent = decompose(magnitude)
      numerator = exponent >= 0 ? mantissa << exponent : mantissa
      denominator = exponent >= 0 ? 1 : 1 << -exponent
      first = numerator.to_s.size - denominator.to_s.size
      first -= 1 until at_least_power?(numerator, denominator, first)
      first += 1 while at_least_power?(numerator, denominator, first + 1)

      (1..17).each do |length|
        rounded = rounded_quotient(*scaled(numerator, denominator, length - 1 - first))
        leading = first
        if rounded == 10**length
          rounded = 10**(length - 1)
          leading += 1
        end
        next unless decimal_to_float(rounded, leading - length + 1) == magnitude

        digits = rounded.to_s
        digits = digits[0...-1] while digits.size > 1 && digits.end_with?("0")
        return [digits, leading]
      end
    end

    # Rust's `str::parse::<f64>` for unsigned decimal text (`3.14`, `5.`,
    # `.5`, `1e5`): the float nearest the exact decimal value, ties to even.
    #
    # @param text [String] digits with an optional `.` and `e` exponent
    # @return [Float]
    def self.parse_float(text)
      mantissa, exponent = text.downcase.split("e", 2)
      whole, fraction = mantissa.split(".", 2)
      fraction ||= ""
      decimal_to_float("#{whole}#{fraction}".to_i, exponent.to_i - fraction.size)
    end

    # The float nearest `digits * 10**exponent`, ties to even.
    #
    # @param digits [Integer] nonnegative
    # @param exponent [Integer]
    # @return [Float]
    def self.decimal_to_float(digits, exponent)
      return 0.0 if digits.zero?

      magnitude = digits.to_s.size + exponent
      return Float::INFINITY if magnitude > 310
      return 0.0 if magnitude < -330

      numerator, denominator = scaled(digits, 1, exponent)
      binary = numerator.to_s(2).size - denominator.to_s(2).size - 53
      binary = -1074 if binary < -1074
      loop do
        mantissa = rounded_quotient(*shifted(numerator, denominator, binary))
        if mantissa >= 2**53
          binary += 1
        elsif mantissa < 2**52 && binary > -1074
          binary -= 1
        else
          return binary > 971 ? Float::INFINITY : Math.ldexp(mantissa.to_f, binary)
        end
      end
    end

    # `value` as `mantissa * 2**exponent` with an integer mantissa below
    # 2**53 and an exponent no lower than a subnormal's -1074.
    #
    # @param value [Float] finite and positive
    # @return [Array(Integer, Integer)]
    def self.decompose(value)
      fraction, exponent = Math.frexp(value)
      mantissa = Math.ldexp(fraction, 53).to_i
      exponent -= 53
      return [mantissa, exponent] if exponent >= -1074

      [mantissa >> (-1074 - exponent), -1074]
    end

    # @return [Boolean] whether `numerator / denominator >= 10**power`
    def self.at_least_power?(numerator, denominator, power)
      scaled_numerator, scaled_denominator = scaled(numerator, denominator, -power)
      scaled_numerator >= scaled_denominator
    end

    # `numerator / denominator` multiplied by `10**power`, kept as integers.
    #
    # @return [Array(Integer, Integer)]
    def self.scaled(numerator, denominator, power)
      power >= 0 ? [numerator * 10**power, denominator] : [numerator, denominator * 10**-power]
    end

    # `numerator / denominator` divided by `2**power`, kept as integers.
    #
    # @return [Array(Integer, Integer)]
    def self.shifted(numerator, denominator, power)
      power >= 0 ? [numerator, denominator << power] : [numerator << -power, denominator]
    end

    # @return [Integer] `numerator / denominator` rounded half to even
    def self.rounded_quotient(numerator, denominator)
      quotient, remainder = numerator.divmod(denominator)
      twice = remainder * 2
      twice > denominator || (twice == denominator && quotient.odd?) ? quotient + 1 : quotient
    end

    # Places `digits` around the decimal point for base-10 `exponent`.
    #
    # @param digits [String]
    # @param exponent [Integer]
    # @return [String]
    def self.positional(digits, exponent)
      if exponent < 0
        "0.#{"0" * (-exponent - 1)}#{digits}"
      elsif digits.size <= exponent + 1
        digits + ("0" * (exponent + 1 - digits.size))
      else
        "#{digits[0..exponent]}.#{digits[(exponent + 1)..]}"
      end
    end

    # @param value [Float]
    # @return [String, nil] Rust's spelling for NaN and infinities
    def self.special_float(value)
      return "NaN" if value.nan?
      return (value > 0 ? "inf" : "-inf") if value.infinite?

      nil
    end

    # @param value [Float]
    # @return [String] `-` for negatives, including negative zero
    def self.sign(value)
      value < 0 || (value.zero? && 1.0 / value < 0) ? "-" : ""
    end
  end
end
