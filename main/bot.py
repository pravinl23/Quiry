import os
import discord
from discord.ext import commands
from dotenv import load_dotenv
from database import store_message, get_server_db
from retrieval import generate_response
import asyncio
from concurrent.futures import ThreadPoolExecutor


executor = ThreadPoolExecutor()

# Load environment variables (the hidden stuff)
load_dotenv()
TOKEN = os.getenv("DISCORD_TOKEN")

# Check for possible error source
if not TOKEN:
    raise ValueError("DISCORD_TOKEN not found")

# Enable intents
intents = discord.Intents.default()
intents.message_content = True

# Dictionary for spam detection
last_messages = {}

bot = commands.Bot(command_prefix='/', intents=intents)

@bot.event
async def on_ready():
    print(f'Logged in as {bot.user}')
    await bot.tree.sync()
    print("Bot is running!")


@bot.event
# Handles incoming messages, stores them in the mongodb database with category tracking
async def on_message(message):
    # Ignore messages from the bot itself or DMs to the bot
    if message.author == bot.user or not message.guild:
        return  

    # If the same user sends an identical message within 10 seconds then the message gets ignored
    current_time = message.created_at.timestamp()
    user_id = message.author.id
    if user_id in last_messages:
        last_content, last_time = last_messages[user_id]
        if message.content == last_content and (current_time - last_time) < 10:
            return
        
    # Update the last message record for the user
    last_messages[user_id] = (message.content, current_time)

    # Discord channels don't need to be in a category so make sure it is in one, otherwise no category
    category = message.channel.category.name if message.channel.category else "No Category"

    # Defining the parameters to store the dataset
    # Pass the actual channel object here:
    store_message(
        server_id=message.guild.id,
        author=str(message.author),
        user_id=message.author.id,
        content=message.content,
        category=category,
        channel=message.channel,  # <-- pass channel object instead of string
        server=str(message.guild.name),
        timestamp=message.created_at
    )

    # Process commands
    await bot.process_commands(message)

# Fetches recent messages and searches for an answer.
@bot.tree.command(name="ask", description="Ask me anything about this server!")
async def ask(interaction: discord.Interaction, question: str):
    # Defer the response immediately to prevent timeout
    await interaction.response.defer(thinking=True)  

    server_id = interaction.guild.id

    try:
        # Run generate_response in an executor to prevent blocking
        loop = asyncio.get_event_loop()
        response = await loop.run_in_executor(executor, generate_response, question, server_id)

        # Send follow-up message once the response is ready
        await interaction.followup.send(response)
    
    except Exception as e:
        await interaction.followup.send(f"An error occurred: {e}", ephemeral=True)

# This function is for admins to clear messages from their servers database, in case of a reset of data, privacy reasons, or if spam is detected
@bot.tree.command(name="clear", description="Clear X amount of recent messages from the database")
async def clear(interaction: discord.Interaction, count: int):
    await interaction.response.defer()
    # This command is for administrators only
    if not interaction.user.guild_permissions.administrator:
        await interaction.response.send_message("You do not have permission to use this command.", ephemeral=True)
        return

    server_id = interaction.guild.id
    db = get_server_db(server_id)
    collection = db["messages"]
    
    # Retrieve the most recent X messages (sorted by descending timestamp)
    messages_to_delete = list(collection.find({}).sort("timestamp", -1).limit(count))
    if not messages_to_delete:
        await interaction.followup.send("No messages found to clear.", ephemeral=True)
        return
    
    # Get the _id values of the messages to delete
    ids = [msg["_id"] for msg in messages_to_delete]
    result = collection.delete_many({"_id": {"$in": ids}})
    await interaction.followup.send(f"Deleted {result.deleted_count} messages from the database.", ephemeral=True)

@bot.tree.command(name="invite", description="Get the bot's invite link!")
async def invite(interaction: discord.Interaction):
    invite_link = "https://discord.com/oauth2/authorize?client_id=1340139928994189322&permissions=8&integration_type=0&scope=bot"
    await interaction.response.send_message(f"Invite me to your server using this link: {invite_link}")


# Too many complications
'''
# This function is for admins to get their bot started when its added to their server, so they have data already to make the bot work
@bot.tree.command(name="fetch", description="Fetches past messages from this server to store in the database")
async def fetch(interaction: discord.Interaction, count: int):
    # This command is for administrators only
    if not interaction.user.guild_permissions.administrator:
        await interaction.response.send_message("You do not have permission to use this command.", ephemeral=True)
        return

    # Defer the interaction to prevent expiration
    await interaction.response.defer(ephemeral=True)

    server_id = interaction.guild.id
    fetched_count = 0

    # Iterate through each text channel
    for channel in interaction.guild.text_channels:
        if fetched_count > count:
            break
        if channel.permissions_for(interaction.guild.me).read_message_history:
            async for message in channel.history(limit=count):
                if message.author.bot:
                    fetched_count = fetched_count - 1
                    continue
                category = None
                if message.channel.category:
                    category = message.channel.category.name
                else:
                    category = "No Category"

                store_message(
                    server_id=server_id,
                    author=str(message.author),
                    user_id=message.author.id,
                    content=message.content,
                    category=category,
                    channel=str(channel),
                    server=str(interaction.guild.name)
                )
                fetched_count = fetched_count + 1
                if fetched_count > count:
                    break

    await interaction.followup.send(f"Fetched and stored {fetched_count} historical messages.", ephemeral=True)
'''


bot.run(TOKEN)
