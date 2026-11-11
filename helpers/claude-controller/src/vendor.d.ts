/** The validator package ships JavaScript without declarations for this entry. */
declare module "validator/lib/isEmail" {
  export default function isEmail(value: string, options: {
    require_tld: boolean; allow_ip_domain: boolean; allow_utf8_local_part: boolean;
  }): boolean;
}
