import grpc from 'k6/net/grpc';
import { check, sleep } from 'k6';

// k6 alternative load test
// Run: k6 run loadtest/k6.js
// Requires k6 with gRPC support (k6 >= 0.29)

export const options = {
  stages: [
    { duration: '10s', target: 50 },
    { duration: '30s', target: 50 },
    { duration: '10s', target: 0 },
  ],
  thresholds: {
    grpc_req_duration: ['p(99)<200'],
  },
};

const client = new grpc.Client();
client.load(['proto'], 'jobqueue.proto');

export default function () {
  client.connect('localhost:50051', { plaintext: true });

  const res = client.invoke('jobqueue.JobQueue/Enqueue', {
    idempotency_key: `k6-${__VU}-${__ITER}`,
    queue: 'load',
    payload: btoa('hello'),
    max_attempts: 3,
  });

  check(res, { 'status OK': (r) => r.status === grpc.StatusOK });
  client.close();
  sleep(0.01);
}
