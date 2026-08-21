output "sites" {
  description = "Each site's custom domain and the Pages deployment behind it."
  value = {
    for k, v in local.sites : k => {
      domain    = v.domain
      pages_dev = cloudflare_pages_project.site[k].subdomain
    }
  }
}
