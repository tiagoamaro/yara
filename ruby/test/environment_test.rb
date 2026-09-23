require "minitest/autorun"
require_relative "../lib/yara"

# Ported from the tests in `rust/src/env.rs`.
class EnvironmentTest < Minitest::Test
  def test_inner_scope_shadows_then_restores
    env = Yara::Environment.new
    env.declare("x", 1)
    env.push_scope
    env.declare("x", 2)

    assert_equal 2, env.lookup("x")
    env.pop_scope
    assert_equal 1, env.lookup("x")
  end

  def test_set_or_declare_mutates_outer_binding_in_place
    env = Yara::Environment.new
    env.declare("x", 1)
    env.push_scope
    env.set_or_declare("x", 5)
    env.pop_scope

    assert_equal 5, env.lookup("x")
  end

  def test_set_or_declare_introduces_new_binding
    env = Yara::Environment.new
    env.set_or_declare("y", 9)

    assert_equal 9, env.lookup("y")
    assert env.current.key?("y")
  end

  def test_unbound_name_is_nil_and_not_bound
    env = Yara::Environment.new

    assert_nil env.lookup("nope")
    refute env.bound?("nope")
  end

  def test_bound_distinguishes_a_nil_value
    env = Yara::Environment.new
    env.declare("x", nil)

    assert env.bound?("x")
  end

  def test_values_collects_all_bindings
    env = Yara::Environment.new
    env.declare("x", 1)
    env.push_scope
    env.declare("y", 2)

    assert_equal [1, 2], env.values.sort
  end
end
