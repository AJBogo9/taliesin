# Everything in this config was created through the Cloudflare API on 2026-08-21, before the
# config was ever applied. These blocks adopt it into state so the first `terraform apply`
# reconciles rather than failing on "already exists".
#
# Why it happened in that order: `wrangler pages` has no domain subcommand, the dashboard was
# unusable (Cloudflare's own bot check blocks a CDP-driven browser, and Google refuses OAuth
# in one), and minting a Terraform token needs the dashboard too. The Cloudflare MCP server
# had API access already, so the work went through that and the config caught up afterwards.
#
# Import ids:
#   cloudflare_pages_project  <account_id>/<project_name>
#   cloudflare_pages_domain   <account_id>/<project_name>/<domain>
#   cloudflare_record         <zone_id>/<record_id>
#
# The project imports live in pages.tf next to their resource. These are the rest.

import {
  for_each = local.sites
  to       = cloudflare_pages_domain.site[each.key]
  id       = "${var.cloudflare_account_id}/${each.value.project}/${each.value.domain}"
}

# Record ids are opaque and only knowable after creation, so they are pinned here rather than
# derived. If a record is ever deleted and recreated outside Terraform, its id changes and
# these need updating; `terraform plan` will say so by proposing a create.
locals {
  dns_record_ids = {
    site      = "db86fc6b3a5c757d221ed400f7d8f729"
    guide     = "cdd26c857b753df2170f56cbf41310d7"
    internals = "f4f4c7a169423a1d04f3fa9658cde17e"
    gallery   = "0fb43c454ffd046785b5e871655f4fd9"
  }
}

import {
  for_each = local.sites
  to       = cloudflare_record.site[each.key]
  id       = "${data.cloudflare_zone.taliesin.id}/${local.dns_record_ids[each.key]}"
}
