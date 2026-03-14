package kafka

import (
	"reflect"
	"testing"
	"time"

	kafkago "github.com/segmentio/kafka-go"
)

func TestMessageFromRecordCopiesKafkaMetadata(t *testing.T) {
	t.Parallel()

	message := messageFromRecord(kafkago.Message{
		Topic:     "sales.order.created",
		Key:       []byte("order-1"),
		Value:     []byte(`{"order_id":"1"}`),
		Headers:   []kafkago.Header{{Key: "content-type", Value: []byte("application/json")}},
		Partition: 2,
		Offset:    30,
		Time:      time.Unix(10, 0).UTC(),
	})

	if message.Topic != "sales.order.created" || message.Offset != 30 {
		t.Fatalf("unexpected mapped message %+v", message)
	}
	if message.Headers["content-type"] != "application/json" {
		t.Fatalf("expected header to be mapped, got %+v", message.Headers)
	}
}

func TestNormalizeValuesDeduplicatesAndSorts(t *testing.T) {
	t.Parallel()

	values := normalizeValues([]string{" kafka:9092 ", "", "kafka:9092", "broker:9092"})
	expected := []string{"broker:9092", "kafka:9092"}
	if !reflect.DeepEqual(values, expected) {
		t.Fatalf("expected normalized values %v, got %v", expected, values)
	}
}
