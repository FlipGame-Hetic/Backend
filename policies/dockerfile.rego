package dockerfile

# Prevent execution as root
deny[msg] {
    input[i].Cmd == ‘user’
    val := input[i].Value
    val[_] == ‘root’
    msg = ‘Containers must not run as root’
}

# Require a USER in the Dockerfile
deny[msg] {
    not any_user
    msg = ‘The Dockerfile must contain a USER instruction’
}

any_user {
    input[i].Cmd == ‘user’
}

# Disallow the :latest tag
deny[msg] {
    input[i].Cmd == ‘from’
    val := input[i].Value
    contains(val[0], ‘:latest’)
    msg = sprintf(‘Avoid the :latest tag; use a specific tag: %s’, [val[0]])
}

# Require a HEALTHCHECK
deny[msg] {
    not any_healthcheck
    msg = ‘The Dockerfile must contain a HEALTHCHECK’
}

any_healthcheck {
    input[i].Cmd == ‘healthcheck’
}
