# frozen_string_literal: true

require 'erb'

class DigestRenderer
  TEMPLATE = %(
    Your daily Hackernews digest:
    <br>
    <br>
    <% for @post in @digest['posts'] %>
      <p>
        <%= @post['title'] %>
        <br>
        <% if @post['url'] %>
          <a href="<%= @post['url'] %>">
            link
          </a> -
        <% end %>
        <a href="https://news.ycombinator.com/item?id=<%= @post['objectID'] %>">
          comments
        </a>
      </p>
    <% end %>
    <br>
    <br>
    Reply to unsubscribe.
  )
  private_constant :TEMPLATE

  def initialize(digest:, date:)
    @date = date
    @digest = digest
  end

  def subject
    "HackerNews Digest for #{@date.getutc.strftime('%b %-d, %Y')}"
  end

  def content
    ERB.new(TEMPLATE, trim_mode: '>-').result(binding)
  end
end
