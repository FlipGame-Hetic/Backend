
# DevOps Security Checklist

## Docker Images
- [ ] Minimal base image (alpine)
- [X] No :latest tag
- [X] Non-root user
- [X] HEALTHCHECK in place
- [ ] Trivy scan with no critical vulnerabilities
- [X] Complete .dockerignore

## Kubernetes
- [ ] Resource limits on each container
- [X] No privileged containers
- [ ] NetworkPolicies in place
- [ ] Secrets via Secret Manager (not in plain text)
- [ ] RBAC configured

## Pipeline
- [ ] Secrets in GitHub Secrets (not in the code)
- [ ] Automatic dependency scan
- [ ] Automatic image scan
- [ ] Secret detection (gitleaks)
- [ ] Environments with protection

## Infrastructure
- [ ] Encrypted Terraform state
- [ ] No hard-coded credentials in IaC files
- [ ] Principle of least privilege (IAM)
