resource "cloudflare_pages_project" "site" {
  for_each = local.sites

  account_id        = var.cloudflare_account_id
  name              = each.value.project
  production_branch = "main"

  lifecycle {
    # The API fills in defaults Terraform never set, so this diffs forever otherwise.
    ignore_changes = [deployment_configs]
  }
}

# The four projects were created with `wrangler pages project create` on 2026-08-20, before
# this config existed. These blocks adopt them into state instead of failing on "already
# exists". They are harmless once the import has happened, and Terraform ignores them on
# subsequent runs, so they can stay as the record of where the projects came from.
import {
  for_each = local.sites
  to       = cloudflare_pages_project.site[each.key]
  id       = "${var.cloudflare_account_id}/${each.value.project}"
}

resource "cloudflare_pages_domain" "site" {
  for_each = local.sites

  account_id   = var.cloudflare_account_id
  project_name = cloudflare_pages_project.site[each.key].name
  domain       = each.value.domain
}
