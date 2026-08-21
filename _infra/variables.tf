variable "cloudflare_api_token" {
  description = "Cloudflare API token. Keep it in terraform.tfvars, which is gitignored."
  type        = string
  sensitive   = true
}

# No default, deliberately. The tech-blog config carries the account id as a default because
# that repository is private; this one is public, so the id lives in terraform.tfvars beside
# the token. It is not a credential, but it identifies the account and nothing here needs it
# to be committed.
variable "cloudflare_account_id" {
  description = "Cloudflare account id that owns the four Pages projects."
  type        = string
}
