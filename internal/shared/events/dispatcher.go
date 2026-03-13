package events

import (
	"context"
	"errors"
	"sync"
)

// Dispatcher delivers events synchronously to in-process handlers.
type Dispatcher struct {
	mu       sync.RWMutex
	handlers map[Name][]Handler
}

func NewDispatcher() *Dispatcher {
	return &Dispatcher{
		handlers: make(map[Name][]Handler),
	}
}

func (d *Dispatcher) Register(name Name, handler Handler) {
	if d == nil || name == "" || handler == nil {
		return
	}

	d.mu.Lock()
	defer d.mu.Unlock()

	d.handlers[name] = append(d.handlers[name], handler)
}

func (d *Dispatcher) Dispatch(ctx context.Context, event Event) error {
	if d == nil {
		return nil
	}

	if prob := Validate(event); prob != nil {
		return prob
	}

	d.mu.RLock()
	handlers := append([]Handler(nil), d.handlers[event.EventName()]...)
	d.mu.RUnlock()

	var errs []error
	for _, handler := range handlers {
		if err := handler.Handle(ctx, event); err != nil {
			errs = append(errs, err)
		}
	}

	return errors.Join(errs...)
}
