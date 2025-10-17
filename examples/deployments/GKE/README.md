# Dynamo Deployment on GKE

## Pre-requisites

### Install gcloud CLI
https://cloud.google.com/sdk/docs/install 

### Create GKE cluster

```bash
export PROJECT_ID=<>
export REGION=<>
export ZONE=<>
export CLUSTER_NAME=<>
export NODE_POOL_MACHINE_TYPE=g2-standard-24
export CLUSTER_MACHINE_TYPE=n2-standard-4
export GPU_TYPE=nvidia-l4
export GPU_COUNT=6
export DISK_SIZE=200

gcloud container clusters create ${CLUSTER_NAME} \
 	--project=${PROJECT_ID} \
 	--location=${ZONE} \
	--subnetwork=default \
    --disk-size=${DISK_SIZE} \
	--machine-type=${CLUSTER_MACHINE_TYPE} \
 	--num-nodes=1
```

#### Create GPU pool

```bash
gcloud container node-pools create gpu-pool \
 	--accelerator type=${GPU_TYPE},count=${GPU_COUNT},gpu-driver-version=latest \
 	--project=${PROJECT_ID} \
 	--location=${ZONE} \
 	--cluster=${CLUSTER_NAME} \
	--machine-type=${NODE_POOL_MACHINE_TYPE} \
    --disk-size=${DISK_SIZE} \
    --service-account=${SA} \
    --num-nodes=1 \
    --enable-autoscaling \
    --min-nodes=1 \
    --max-nodes=3
```

###  Install helm

```bash
curl https://baltocdn.com/helm/signing.asc | sudo tee /etc/apt/trusted.gpg.d/helm.asc > /dev/null
echo "deb https://baltocdn.com/helm/stable/debian/ all main" | sudo tee /etc/apt/sources.list.d/helm-stable-debian.list

sudo apt-get update
sudo apt-get install -y helm
```

###  Clone Dynamo GitHub repository

**Note:** Please make sure GitHub branch/commit version matches with Dynamo platform and VLLM container. 

```bash
git clone https://github.com/ai-dynamo/dynamo.git

# Checkout to the desired branch
git checkout release/0.4.0
```

###  Set environment variables for GKE

```bash
export NAMESPACE=dynamo-cloud
kubectl create namespace $NAMESPACE
kubectl config set-context --current --namespace=$NAMESPACE

export HF_TOKEN=<HF_TOKEN>
kubectl create secret generic hf-token-secret \
  --from-literal=HF_TOKEN=${HF_TOKEN} \
  -n ${NAMESPACE}
```

## Install Dynamo Kubernetes Platform

### Path 1: Production Deployment (Selected)

```bash
# 1. Set environment
export NAMESPACE=dynamo-cloud
export RELEASE_VERSION=0.4.0 # any version of Dynamo 0.3.2+

# 2. Install CRDs
helm fetch https://helm.ngc.nvidia.com/nvidia/ai-dynamo/charts/dynamo-crds-${RELEASE_VERSION}.tgz
helm install dynamo-crds dynamo-crds-${RELEASE_VERSION}.tgz \
  --namespace default \
  --wait \
  --atomic

# 3. Install Platform
helm fetch https://helm.ngc.nvidia.com/nvidia/ai-dynamo/charts/dynamo-platform-${RELEASE_VERSION}.tgz
helm install dynamo-platform dynamo-platform-${RELEASE_VERSION}.tgz --namespace ${NAMESPACE}
```

### Path 2: Custom Deployment

```bash
cd dynamo/deploy/cloud/helm

# Install Custom Resource Definitions (CRDs)
helm install dynamo-crds ./crds/ \
  --namespace default \
  --wait \
  --atomic

helm dep build ./platform/

# Install platform
helm install dynamo-platform ./platform/ \
  --namespace ${NAMESPACE} \
  --set "dynamo-operator.controllerManager.manager.image.repository=${DOCKER_SERVER}/dynamo-operator" \
  --set "dynamo-operator.controllerManager.manager.image.tag=${IMAGE_TAG}" \
  --set "dynamo-operator.imagePullSecrets[0].name=docker-imagepullsecret"
```

**Expected output**

```bash
kubectl get pods
NAME                                                              READY   STATUS             RESTARTS   AGE
dynamo-platform-dynamo-operator-controller-manager-69b9794fpgv9   2/2     Running            0          4m27s
dynamo-platform-etcd-0                                            1/1     Running            0          4m27s
dynamo-platform-nats-0                                            2/2     Running            0          4m27s
dynamo-platform-nats-box-5dbf45c748-ql2nk                         1/1     Running            0          4m27s
```

Other ways to install Dynamo platform could be found here https://github.com/ai-dynamo/dynamo/blob/main/docs/guides/dynamo_deploy/dynamo_cloud.md 

## Deploy Inference Graph

We will deploy a LLM model to the Dynamo platform. Here we use `google/gemma-3-1b-it` model with VLLM and disaggregated deployment as an example. 

In the deployment yaml file, some adjustments have to/ could be made:

- **(Required)** Add args to change `LD_LIBRARY_PATH` and `PATH` of decoder container, to enable GKE find the correct GPU driver
- Change VLLM  image to the desired one on NGC
- Add namespace to metadata
- Adjust GPU/CPU request and limits
- Change model to deploy

More configurations please refer to https://github.com/ai-dynamo/dynamo/tree/main/examples/deployments/GKE/vllm

### Highlighted configurations in yaml file
Please note that `LD_LIBRARY_PATH` needs to be set properly in GKE as per [Run GPUs in GKE](https://cloud.google.com/kubernetes-engine/docs/how-to/gpus)

The following snippet needs to be present in the `args` field of the deployment `yaml` file:

```bash
export LD_LIBRARY_PATH=/usr/local/nvidia/lib64:$LD_LIBRARY_PATH
export PATH=$PATH:/usr/local/nvidia/bin:/usr/local/nvidia/lib64
/sbin/ldconfig
```

For example, refer to the following from [`examples/deployments/GKE/vllm/disagg_gke.yaml`](./vllm/disagg_gke.yaml)

```yaml
metadata:
  name: vllm-disagg
  namespace: dynamo-cloud
spec:
  services:
    Frontend:
          image: nvcr.io/nvidia/ai-dynamo/vllm-runtime:0.4.0
    VllmDecodeWorker:
​​      resources:
        limits:
          gpu: "3"
          image: nvcr.io/nvidia/ai-dynamo/vllm-runtime:0.4.0
          args:
            - |
            export LD_LIBRARY_PATH=/usr/local/nvidia/lib64:$LD_LIBRARY_PATH
            export PATH=$PATH:/usr/local/nvidia/bin:/usr/local/nvidia/lib64
            /sbin/ldconfig
            python3 -m dynamo.vllm --model google/gemma-3-1b-it
```

## Deploy the model

```bash
cd dynamo/examples/deployments/GKE/vllm

kubectl apply -f disagg_gke.yaml
```

**Expected output after successful deployment**

```bash
kubectl get pods
NAME                                                              READY   STATUS    RESTARTS   AGE
dynamo-platform-dynamo-operator-controller-manager-c665684ssqkx   2/2     Running   0          65m
dynamo-platform-etcd-0                                            1/1     Running   0          65m
dynamo-platform-nats-0                                            2/2     Running   0          65m
dynamo-platform-nats-box-5dbf45c748-rbwjr                         1/1     Running   0          65m
vllm-disagg-frontend-5954ddc4dd-4w2cb                             1/1     Running   0          11m
vllm-disagg-vllmdecodeworker-77844cfcff-ddn4v                     1/1     Running   0          11m
vllm-disagg-vllmprefillworker-55d5b74b4f-zrskh                    1/1     Running   0          11m
```

## Test the Deployment

```bash
export DEPLOYMENT_NAME=vllm-disagg

# Find the frontend pod
export FRONTEND_POD=$(kubectl get pods -n ${NAMESPACE} | grep "${DEPLOYMENT_NAME}-frontend" | sort -k1 | tail -n1 | awk '{print $1}')

# Forward the pod's port to localhost
kubectl port-forward deployment/vllm-disagg-frontend  8000:8000 -n ${NAMESPACE}

# disagg
curl localhost:8000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "google/gemma-3-1b-it",
    "messages": [
    {
        "role": "user",
        "content": "In the heart of Eldoria, an ancient land of boundless magic and mysterious creatures, lies the long-forgotten city of Aeloria. Once a beacon of knowledge and power, Aeloria was buried beneath the shifting sands of time, lost to the world for centuries. You are an intrepid explorer, known for your unparalleled curiosity and courage, who has stumbled upon an ancient map hinting at ests that Aeloria holds a secret so profound that it has the potential to reshape the very fabric of reality. Your journey will take you through treacherous deserts, enchanted forests, and across perilous mountain ranges. Your Task: Character Background: Develop a detailed background for your character. Describe their motivations for seeking out Aeloria, their skills and weaknesses, and any personal connections to the ancient city or its legends. Are they driven by a quest for knowledge, a search for lost familt clue is hidden."
    }
    ],
    "stream":false,
    "max_tokens": 30
  }'
```

### Response

```json
{"id":"chatcmpl-bd0670d9-0342-4eea-97c1-99b69f1f931f","choices":[{"index":0,"message":{"content":"Okay, here’s a detailed character background for your intrepid explorer, tailored to fit the premise of Aeloria, with a focus on a","refusal":null,"tool_calls":null,"role":"assistant","function_call":null,"audio":null},"finish_reason":"stop","logprobs":null}],"created":1756336263,"model":"google/gemma-3-1b-it","service_tier":null,"system_fingerprint":null,"object":"chat.completion","usage":{"prompt_tokens":190,"completion_tokens":29,"total_tokens":219,"prompt_tokens_details":null,"completion_tokens_details":null}}
```
