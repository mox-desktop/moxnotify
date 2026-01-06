local_resource(
    'run-redis',
    serve_cmd='valkey-server'
)

local_resource(
    'run-control-plane',
    cmd='cargo build --bin control_plane',
    serve_cmd='cargo run --bin control_plane',
    resource_deps=["run-redis"]
)

local_resource(
    'run-collector',
    cmd='cargo build --bin collector-dbus',
    serve_cmd='cargo run --bin collector-dbus',
    resource_deps=['run-control-plane']
)

local_resource(
    'run-indexer',
    cmd='cargo build --bin indexer',
    serve_cmd='cargo run --bin indexer',
    resource_deps=['run-control-plane']
)

local_resource(
    'run-searcher',
    cmd='cargo build --bin searcher',
    serve_cmd='cargo run --bin searcher'
)

local_resource(
    'run-scheduler',
    cmd='cargo build --bin scheduler',
    serve_cmd='cargo run --bin scheduler',
    resource_deps=['run-control-plane']
)

local_resource(
    'run-client',
    cmd='cargo build --bin client',
    serve_cmd='cargo run --bin client',
    resource_deps=['run-scheduler']
)

local_resource(
    'run-janitor',
    cmd='cargo build --bin janitor',
    serve_cmd='cargo run --bin janitor',
    resource_deps=['run-indexer']
)

local_resource(
    'run-webui',
    cmd='pnpm --dir webui i',
    serve_cmd='pnpm --dir webui dev',
    resource_deps=['run-searcher']
)
