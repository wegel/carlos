/** Uses established validators for MCP's four documented string formats. */
import { fullFormats } from "ajv-formats/dist/formats";
import isEmail from "validator/lib/isEmail";

/** Check format syntax without normalizing the user's value. */
export function matchesFormat(value: string, format: string): boolean {
  // RFC mailboxes do not require a dotted domain. Do not apply provider-specific rules.
  if (format === "email") return isEmail(value, { require_tld: false, allow_ip_domain: true, allow_utf8_local_part: false });
  if (format !== "uri" && format !== "date" && format !== "date-time") return false;
  const rule = fullFormats[format];
  if (rule instanceof RegExp) return rule.test(value);
  if (typeof rule === "function") return Boolean(rule(value));
  if (typeof rule === "object") {
    const validate = rule.validate;
    return validate instanceof RegExp ? validate.test(value) : typeof validate === "function" && Boolean((validate as (text: string) => boolean)(value));
  }
  return false;
}
