terraform {
  required_providers {
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 4"
    }
  }
  # `import` blocks with `for_each` need 1.7+.
  required_version = ">= 1.7"
}

provider "cloudflare" {
  api_token = var.cloudflare_api_token
}

# The four sites. One Taliesin project, one Cloudflare Pages project and one domain each;
# nothing is composed across them, because Pages has no subpath deploy and every deployment
# is a whole-site snapshot. `tools/publish.sh` is the thing that uploads content: these
# projects are DIRECT UPLOAD, so there is deliberately no `source` block below and no build
# runs on Cloudflare's side.
locals {
  sites = {
    site = {
      project = "taliesin-site"
      domain  = "taliesin.sh"
      # Apex. Cloudflare flattens a CNAME here, the same way the tech-blog zone does.
      record = "taliesin.sh"
    }
    guide = {
      project = "taliesin-guide"
      domain  = "guide.taliesin.sh"
      record  = "guide"
    }
    internals = {
      project = "taliesin-internals"
      domain  = "internals.taliesin.sh"
      record  = "internals"
    }
    gallery = {
      project = "taliesin-gallery"
      domain  = "gallery.taliesin.sh"
      record  = "gallery"
    }
  }
}

# Looked up rather than pinned, so no zone id is committed to a public repository.
data "cloudflare_zone" "taliesin" {
  name = "taliesin.sh"
}
