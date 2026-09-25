/* Standalone `yara`: runs the mrbc-compiled Ruby implementation with ARGV. */
#include <mruby.h>
#include <mruby/array.h>
#include <mruby/irep.h>
#include <mruby/string.h>
#include <mruby/variable.h>

extern const uint8_t yara_bytecode[];

int main(int argc, char **argv)
{
  mrb_state *mrb = mrb_open();
  if (mrb == NULL) {
    return 1;
  }

  mrb_value args = mrb_ary_new_capa(mrb, argc - 1);
  for (int i = 1; i < argc; i++) {
    mrb_ary_push(mrb, args, mrb_str_new_cstr(mrb, argv[i]));
  }
  mrb_define_global_const(mrb, "ARGV", args);

  mrb_load_irep(mrb, yara_bytecode);
  int status = 1;
  if (mrb->exc) {
    mrb_print_error(mrb);
  } else {
    mrb_value result = mrb_gv_get(mrb, mrb_intern_lit(mrb, "$yara_status"));
    if (mrb_integer_p(result)) {
      status = (int)mrb_integer(result);
    }
  }
  mrb_close(mrb);
  return status;
}
