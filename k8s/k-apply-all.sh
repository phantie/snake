kubectl apply -f backend/clip.fe.yml
kubectl apply -f backend/dep.yml
kubectl apply -f frontend/clip.fe.yml
kubectl apply -f frontend/dep.yml
kubectl apply -f k8s/ngx.ing.yml