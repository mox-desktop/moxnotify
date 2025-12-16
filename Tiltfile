print("""
-----------------------------------------------------------------
✨ Hello Tilt! This appears in the (Tiltfile) pane whenever Tilt
   evaluates this file.
-----------------------------------------------------------------
""".strip())
warn('ℹ️ Open {tiltfile_path} in your favorite editor to get started.'.format(
    tiltfile_path=config.main_path))


# Build Docker image
#   Tilt will automatically associate image builds with the resource(s)
#   that reference them (e.g. via Kubernetes or Docker Compose YAML).
#
#   More info: https://docs.tilt.dev/api.html#api.docker_build
#
# docker_build('registry.example.com/my-image',
#              context='.',
#              # (Optional) Use a custom Dockerfile path
#              dockerfile='./deploy/app.dockerfile',
#              # (Optional) Filter the paths used in the build
#              only=['./app'],
#              # (Recommended) Updating a running container in-place
#              # https://docs.tilt.dev/live_update_reference.html
#              live_update=[
#                 # Sync files from host to container
#                 sync('./app', '/src/'),
#                 # Execute commands inside the container when certain
#                 # paths change
#                 run('/src/codegen.sh', trigger=['./app/api'])
#              ]
# )


# Apply Kubernetes manifests
#   Tilt will build & push any necessary images, re-deploying your
#   resources as they change.
#
#   More info: https://docs.tilt.dev/api.html#api.k8s_yaml
#
# k8s_yaml(['k8s/deployment.yaml', 'k8s/service.yaml'])


# Customize a Kubernetes resource
#   By default, Kubernetes resource names are automatically assigned
#   based on objects in the YAML manifests, e.g. Deployment name.
#
#   Tilt strives for sane defaults, so calling k8s_resource is
#   optional, and you only need to pass the arguments you want to
#   override.
#
#   More info: https://docs.tilt.dev/api.html#api.k8s_resource
#
# k8s_resource('my-deployment',
#              # map one or more local ports to ports on your Pod
#              port_forwards=['5000:8080'],
#              # change whether the resource is started by default
#              auto_init=False,
#              # control whether the resource automatically updates
#              trigger_mode=TRIGGER_MODE_MANUAL
# )



local_resource(
    'run-control-plane',
    cmd='cargo build --bin control_plane',
    serve_cmd='cargo run --bin control_plane'
)

local_resource(
    'run-collector',
    cmd='cargo build --bin collector',
    serve_cmd='cargo run --bin collector',
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
    'run-webui',
    cmd='pnpm --dir webui i',
    serve_cmd='pnpm --dir webui dev',
    resource_deps=['run-searcher']
)

local_resource(
    'run-redis',
    serve_cmd='valkey-server'
)

