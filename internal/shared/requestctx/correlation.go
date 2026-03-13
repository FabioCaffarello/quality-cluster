package requestctx

import "context"

type correlationIDKey struct{}

func WithCorrelationID(ctx context.Context, correlationID string) context.Context {
	if ctx == nil || correlationID == "" {
		return ctx
	}
	return context.WithValue(ctx, correlationIDKey{}, correlationID)
}

func CorrelationID(ctx context.Context) string {
	if ctx == nil {
		return ""
	}

	value, _ := ctx.Value(correlationIDKey{}).(string)
	return value
}
