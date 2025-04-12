FROM ruby:3.4.2-slim

WORKDIR /app

# Install only what's needed to install the gem, then clean up in the same layer
RUN apt-get update && \
  apt-get install -y --no-install-recommends libclang-dev build-essential curl && \
  gem install itsi --no-document && \
  apt-get purge -y --auto-remove build-essential && \
  rm -rf /var/lib/apt/lists/* /tmp/* /var/tmp/*

ENTRYPOINT ["itsi"]
