import { useEffect, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Layout } from '@/components/layout/Layout';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { Skeleton } from '@/components/ui/skeleton';
import { apiClient } from '@/lib/api';
import { CreateEventRequest, EventType, TicketType, UpdateEventRequest } from '@/types';
import { useAuth } from '@/contexts/AuthContext';
import { toast } from 'sonner';
import { Calendar, MapPin, Image, Ticket, Plus, Trash2, ArrowLeft, Save, Eye } from 'lucide-react';

interface TicketTypeForm {
  id?: string;
  name: string;
  price: string;
  capacity: string;
}

const CreateEventPage = () => {
  const { eventId } = useParams();
  const { user, isAuthenticated } = useAuth();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const isEditing = !!eventId;

  const [formData, setFormData] = useState({
    title: '',
    description: '',
    venue: '',
    location: '',
    startDate: '',
    startTime: '',
    endDate: '',
    endTime: '',
  });

  const [ticketTypes, setTicketTypes] = useState<TicketTypeForm[]>([
    { name: 'General Admission', price: '0', capacity: '100' },
  ]);

  const { data: existingEvent, isLoading: loadingEvent } = useQuery({
    queryKey: ['event', eventId],
    queryFn: () => apiClient.getEvent(eventId!),
    enabled: isEditing && !!eventId,
  });

  useEffect(() => {
    if (existingEvent?.event) {
      const event = existingEvent.event;
      const startDateTime = new Date(event.start_time);
      const endDateTime = new Date(event.end_time);

      const formatDate = (date: Date) => {
        if (isNaN(date.getTime())) return '';
        return date.toISOString().split('T')[0];
      };

      const formatTime = (date: Date) => {
        if (isNaN(date.getTime())) return '';
        return date.toTimeString().slice(0, 5);
      };

      setFormData({
        title: event.title || '',
        description: event.description || '',
        venue: event.venue || '',
        location: event.location || '',
        startDate: formatDate(startDateTime),
        startTime: formatTime(startDateTime),
        endDate: formatDate(endDateTime),
        endTime: formatTime(endDateTime),
      });

      if (existingEvent.ticket_types && existingEvent.ticket_types.length > 0) {
        setTicketTypes(
          existingEvent.ticket_types.map((tt: TicketType) => ({
            id: tt.id,
            name: tt.name,
            price: tt.price?.toString() || '0',
            capacity: tt.capacity?.toString() || '50',
          }))
        );
      }
    }
  }, [existingEvent]);

  const createMutation = useMutation<EventType, Error, CreateEventRequest>({
    mutationFn: (event: CreateEventRequest) => apiClient.createEvent(event),
    onSuccess: createdEvent => {
      queryClient.invalidateQueries({ queryKey: ['organizer-events'] });
      toast.success('Event created successfully!');
      navigate(`/organizer/events/${createdEvent.id}/edit`);
    },
    onError: (error: Error) => {
      toast.error(error.message || 'Failed to create event');
    },
  });

  const updateMutation = useMutation<EventType, Error, UpdateEventRequest>({
    mutationFn: (event: UpdateEventRequest) => apiClient.updateEvent(eventId!, event),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['organizer-events'] });
      queryClient.invalidateQueries({ queryKey: ['event', eventId] });
      toast.success('Event updated successfully!');
    },
    onError: (error: Error) => {
      toast.error(error.message || 'Failed to update event');
    },
  });

  const publishMutation = useMutation<void, Error, string>({
    mutationFn: (id: string) => apiClient.publishEvent(id).then(() => {}),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['organizer-events'] });
      queryClient.invalidateQueries({ queryKey: ['event', eventId] });
      toast.success('Event published successfully!');
      navigate('/organizer/events');
    },
    onError: (error: Error) => {
      toast.error(error.message || 'Failed to publish event');
    },
  });

  const createTicketTypeMutation = useMutation<
    TicketType,
    Error,
    { eventId: string; name: string; price: number; capacity: number }
  >({
    mutationFn: ({ eventId, name, price, capacity }) =>
      apiClient.createTicketType(eventId, { name, price, capacity }),
    onError: (error: Error) => {
      toast.error(error.message || 'Failed to create ticket type');
    },
  });

  const updateTicketTypeMutation = useMutation<
    void,
    Error,
    { eventId: string; ticketTypeId: string; name?: string; price?: number; capacity?: number }
  >({
    mutationFn: ({ eventId, ticketTypeId, name, price, capacity }) =>
      apiClient.updateTicketType(eventId, ticketTypeId, { name, price, capacity }),
    onError: (error: Error) => {
      toast.error(error.message || 'Failed to update ticket type');
    },
  });

  const deleteTicketTypeMutation = useMutation<
    void,
    Error,
    { eventId: string; ticketTypeId: string }
  >({
    mutationFn: ({ eventId, ticketTypeId }) => apiClient.deleteTicketType(eventId, ticketTypeId),
    onError: (error: Error) => {
      toast.error(error.message || 'Failed to delete ticket type');
    },
  });

  if (!isAuthenticated || user?.role !== 'ORGANIZER') {
    return (
      <Layout>
        <div className="container py-16 text-center">
          <h1 className="mb-4 text-2xl font-bold">Organizer Access Required</h1>
          <p className="mb-4 text-muted-foreground">Please sign in with a organizer account.</p>
        </div>
      </Layout>
    );
  }

  if (isEditing && loadingEvent) {
    return (
      <Layout showFooter={false}>
        <div className="container max-w-4xl py-8 lg:py-12">
          <Skeleton className="mb-8 h-12 w-64" />
          <div className="space-y-6">
            <Skeleton className="h-48 w-full" />
            <Skeleton className="h-48 w-full" />
          </div>
        </div>
      </Layout>
    );
  }

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>) => {
    const { name, value } = e.target;
    setFormData(prev => ({ ...prev, [name]: value }));
  };

  const addTicketType = () => {
    setTicketTypes(prev => [
      ...prev,
      {
        name: '',
        price: '0',
        capacity: '50',
      },
    ]);
  };

  const removeTicketType = (index: number) => {
    if (ticketTypes.length === 1) {
      toast.error('At least one ticket type is required');
      return;
    }
    setTicketTypes(prev => prev.filter((_, i) => i !== index));
  };

  const updateTicketType = (index: number, field: string, value: string) => {
    setTicketTypes(prev => prev.map((t, i) => (i === index ? { ...t, [field]: value } : t)));
  };

  const handleSubmit = async (e: React.FormEvent, asDraft = false) => {
    e.preventDefault();

    const startTime = `${formData.startDate}T${formData.startTime}:00`;
    const endTime = `${formData.endDate}T${formData.endTime}:00`;

    const eventData = {
      title: formData.title,
      description: formData.description || undefined,
      venue: formData.venue || undefined,
      location: formData.location || undefined,
      start_time: startTime,
      end_time: endTime,
    };

    try {
      if (isEditing) {
        await updateMutation.mutateAsync(eventData);

        const existingTicketTypes = existingEvent?.ticket_types || [];
        const existingIds = new Set(existingTicketTypes.map((tt: TicketType) => tt.id));
        const currentIds = new Set(ticketTypes.filter(tt => tt.id).map(tt => tt.id));

        for (const tt of ticketTypes) {
          if (tt.id) {
            const existing = existingTicketTypes.find((et: TicketType) => et.id === tt.id);
            if (existing) {
              const nameChanged = existing.name !== tt.name;
              const priceChanged = existing.price !== (parseFloat(tt.price) || 0);
              const capacityChanged = existing.capacity !== (parseInt(tt.capacity) || 0);

              if (nameChanged || priceChanged || capacityChanged) {
                await updateTicketTypeMutation.mutateAsync({
                  eventId: eventId!,
                  ticketTypeId: tt.id,
                  name: tt.name,
                  price: parseFloat(tt.price) || 0,
                  capacity: parseInt(tt.capacity) || 0,
                });
              }
            }
          } else {
            await createTicketTypeMutation.mutateAsync({
              eventId: eventId!,
              name: tt.name,
              price: parseFloat(tt.price) || 0,
              capacity: parseInt(tt.capacity) || 50,
            });
          }
        }

        for (const existing of existingTicketTypes) {
          if (!currentIds.has(existing.id)) {
            await deleteTicketTypeMutation.mutateAsync({
              eventId: eventId!,
              ticketTypeId: existing.id,
            });
          }
        }

        queryClient.invalidateQueries({ queryKey: ['organizer-events'] });
        queryClient.invalidateQueries({ queryKey: ['event', eventId] });

        if (!asDraft) {
          await publishMutation.mutateAsync(eventId!);
        } else {
          toast.success('Event saved as draft!');
          navigate('/organizer/events');
        }
      } else {
        const createdEvent = await createMutation.mutateAsync(eventData);

        for (const tt of ticketTypes) {
          await apiClient.createTicketType(createdEvent.id, {
            name: tt.name,
            price: parseFloat(tt.price) || 0,
            capacity: parseInt(tt.capacity) || 50,
          });
        }

        if (!asDraft) {
          await apiClient.publishEvent(createdEvent.id.toString());
        }

        toast.success(asDraft ? 'Event saved as draft!' : 'Event published successfully!');
        navigate('/organizer/events');
      }
    } catch (error) {
      console.error('Failed to save event:', error);
    }
  };

  return (
    <Layout showFooter={false}>
      <div className="container max-w-4xl py-8 lg:py-12">
        {/* Header */}
        <div className="mb-8 flex items-center gap-4">
          <Button variant="ghost" size="icon" onClick={() => navigate('/organizer/events')}>
            <ArrowLeft className="h-5 w-5" />
          </Button>
          <div>
            <h1 className="text-3xl font-bold">{isEditing ? 'Edit Event' : 'Create Event'}</h1>
            <p className="mt-1 text-muted-foreground">
              Fill in the details to {isEditing ? 'update' : 'create'} your event
            </p>
          </div>
        </div>

        <form onSubmit={e => handleSubmit(e, false)}>
          <div className="space-y-6">
            {/* Basic Info */}
            <Card>
              <CardHeader>
                <CardTitle>Event Details</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="title">Event Title *</Label>
                  <Input
                    id="title"
                    name="title"
                    value={formData.title}
                    onChange={handleInputChange}
                    placeholder="Give your event a catchy title"
                    required
                  />
                </div>

                <div className="space-y-2">
                  <Label htmlFor="description">Description *</Label>
                  <Textarea
                    id="description"
                    name="description"
                    value={formData.description}
                    onChange={handleInputChange}
                    placeholder="Describe what attendees can expect..."
                    rows={5}
                    required
                  />
                </div>
              </CardContent>
            </Card>

            {/* Date & Time */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Calendar className="h-5 w-5" />
                  Date & Time
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid gap-4 sm:grid-cols-2">
                  <div className="space-y-2">
                    <Label htmlFor="startDate">Start Date *</Label>
                    <Input
                      id="startDate"
                      name="startDate"
                      type="date"
                      value={formData.startDate}
                      onChange={handleInputChange}
                      required
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="startTime">Start Time *</Label>
                    <Input
                      id="startTime"
                      name="startTime"
                      type="time"
                      value={formData.startTime}
                      onChange={handleInputChange}
                      required
                    />
                  </div>
                </div>
                <div className="grid gap-4 sm:grid-cols-2">
                  <div className="space-y-2">
                    <Label htmlFor="endDate">End Date *</Label>
                    <Input
                      id="endDate"
                      name="endDate"
                      type="date"
                      value={formData.endDate}
                      onChange={handleInputChange}
                      required
                    />
                  </div>
                  <div className="space-y-2">
                    <Label htmlFor="endTime">End Time *</Label>
                    <Input
                      id="endTime"
                      name="endTime"
                      type="time"
                      value={formData.endTime}
                      onChange={handleInputChange}
                      required
                    />
                  </div>
                </div>
              </CardContent>
            </Card>

            {/* Location */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <MapPin className="h-5 w-5" />
                  Location
                </CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="venue">Venue *</Label>
                  <Input
                    id="venue"
                    name="venue"
                    value={formData.venue}
                    onChange={handleInputChange}
                    placeholder="Enter the venue name and address"
                    required
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="location">Location (City/Region)</Label>
                  <Input
                    id="location"
                    name="location"
                    value={formData.location}
                    onChange={handleInputChange}
                    placeholder="e.g., Nairobi, Kenya"
                  />
                </div>
              </CardContent>
            </Card>

            {/* Tickets */}
            <Card>
              <CardHeader className="flex flex-row items-center justify-between">
                <CardTitle className="flex items-center gap-2">
                  <Ticket className="h-5 w-5" />
                  Ticket Types
                </CardTitle>
                <Button type="button" variant="outline" size="sm" onClick={addTicketType}>
                  <Plus className="mr-2 h-4 w-4" />
                  Add Type
                </Button>
              </CardHeader>
              <CardContent className="space-y-4">
                {ticketTypes.map((ticket, index) => (
                  <div key={index} className="space-y-4 rounded-lg border p-4">
                    <div className="flex items-center justify-between">
                      <span className="text-sm font-medium text-muted-foreground">
                        Ticket Type {index + 1}
                      </span>
                      <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        onClick={() => removeTicketType(index)}
                        className="text-muted-foreground hover:text-destructive"
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </div>
                    <div className="grid gap-4 sm:grid-cols-3">
                      <div className="space-y-2">
                        <Label>Name *</Label>
                        <Input
                          value={ticket.name}
                          onChange={e => updateTicketType(index, 'name', e.target.value)}
                          placeholder="e.g., VIP, Early Bird"
                          required
                        />
                      </div>
                      <div className="space-y-2">
                        <Label>Price ($) *</Label>
                        <Input
                          type="number"
                          min="0"
                          step="0.01"
                          value={ticket.price}
                          onChange={e => updateTicketType(index, 'price', e.target.value)}
                          required
                        />
                      </div>
                      <div className="space-y-2">
                        <Label>Capacity *</Label>
                        <Input
                          type="number"
                          min="1"
                          value={ticket.capacity}
                          onChange={e => updateTicketType(index, 'capacity', e.target.value)}
                          required
                        />
                      </div>
                    </div>
                  </div>
                ))}
              </CardContent>
            </Card>

            {/* Actions */}
            <div className="flex flex-col gap-4 pt-4 sm:flex-row">
              <Button
                type="button"
                variant="outline"
                onClick={() =>
                  handleSubmit(new Event('submit') as unknown as React.FormEvent, true)
                }
                disabled={
                  createMutation.isPending ||
                  updateMutation.isPending ||
                  createTicketTypeMutation.isPending ||
                  updateTicketTypeMutation.isPending ||
                  deleteTicketTypeMutation.isPending
                }
                className="flex-1 sm:flex-none"
              >
                <Save className="mr-2 h-4 w-4" />
                Save as Draft
              </Button>
              <Button
                type="submit"
                disabled={
                  createMutation.isPending ||
                  updateMutation.isPending ||
                  publishMutation.isPending ||
                  createTicketTypeMutation.isPending ||
                  updateTicketTypeMutation.isPending ||
                  deleteTicketTypeMutation.isPending
                }
                className="flex-1 sm:flex-none"
              >
                {createMutation.isPending ||
                updateMutation.isPending ||
                publishMutation.isPending ||
                createTicketTypeMutation.isPending ||
                updateTicketTypeMutation.isPending ||
                deleteTicketTypeMutation.isPending ? (
                  'Saving...'
                ) : (
                  <>
                    <Eye className="mr-2 h-4 w-4" />
                    {isEditing ? 'Update & Publish' : 'Publish Event'}
                  </>
                )}
              </Button>
            </div>
          </div>
        </form>
      </div>
    </Layout>
  );
};

export default CreateEventPage;
