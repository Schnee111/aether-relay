export type EventStatus = 'RECEIVED' | 'PROCESSING' | 'DELIVERED' | 'FAILED' | 'DEAD';
export type CircuitState = 'CLOSED' | 'OPEN' | 'HALF_OPEN';
export type ProviderType = 'github' | 'stripe' | 'midtrans' | 'discord' | 'generic';

export interface EndpointRow {
  id: string;
  name: string;
  target_url: string;
  provider_type: ProviderType;
  secret_key: string;
  timeout_ms: number;
  max_retries: number;
  circuit_state: CircuitState;
  circuit_failures: number;
  circuit_reset_at: number | null;
  created_at: number;
}

export interface IncomingEventRow {
  id: string;
  endpoint_id: string;
  idempotency_key: string;
  status: EventStatus;
  headers_json: string;
  payload_json: string;
  raw_payload: Buffer;
  attempts_count: number;
  next_attempt_at: number;
  locked_until: number | null;
  created_at: number;
}

export interface DeliveryAttemptRow {
  id: string;
  event_id: string;
  attempt_number: number;
  response_status: number | null;
  response_body: string | null;
  error_message: string | null;
  execution_duration_ms: number;
  attempted_at: number;
}

export interface DeadLetterRow {
  id: string;
  event_id: string;
  endpoint_id: string;
  final_error: string;
  replayed_at: number | null;
  replayed_by: string | null;
  created_at: number;
}

export interface DatabaseSchema {
  endpoints: EndpointRow;
  incoming_events: IncomingEventRow;
  delivery_attempts: DeliveryAttemptRow;
  dead_letter_queue: DeadLetterRow;
}
