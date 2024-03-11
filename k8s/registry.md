Create registry
---------------
    doctl registry create snake

Give access to cluster
----------------------
    doctl registry kubernetes-manifest | kubectl apply -f -
    kubectl patch serviceaccount default -p '{"imagePullSecrets": [{"name": "registry-snake"}]}'

Tag built loaded images
-----------------------
    docker tag snake_fe:0.1 registry.digitalocean.com/snake/snake_fe:latest
    docker tag snake_be:0.1 registry.digitalocean.com/snake/snake_be:latest

Push images to the registry
---------------------------
    docker push registry.digitalocean.com/snake/snake_fe:latest
    docker push registry.digitalocean.com/snake/snake_be:latest