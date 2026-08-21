# One proxied CNAME per site, pointing at that Pages project's own *.pages.dev subdomain.
#
# These are what actually make the sites reachable. Until they exist, all four deployments
# are live on *.pages.dev but cannot reach each other: the navbar links across sites are
# ABSOLUTE urls (https://guide.taliesin.sh/... and so on), because separate Pages projects
# have no shared origin to be relative to. `crates/core/tests/cross_site_links.rs` resolves
# every one of those urls against the source tree, so they are correct by construction and
# only DNS is missing.
resource "cloudflare_record" "site" {
  for_each = local.sites

  zone_id = data.cloudflare_zone.taliesin.id
  name    = each.value.record
  type    = "CNAME"
  content = cloudflare_pages_project.site[each.key].subdomain
  proxied = true

  # Cloudflare creates this record itself when a custom domain is attached through the
  # dashboard. Declaring it here means Terraform owns it instead, so depend on the domain
  # to keep the ordering deterministic.
  depends_on = [cloudflare_pages_domain.site]
}
